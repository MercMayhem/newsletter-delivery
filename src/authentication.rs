use actix_web::web;
use argon2::PasswordHasher;
use anyhow::Context;
use argon2::{password_hash::SaltString, Algorithm, Argon2, Params, PasswordHash, PasswordVerifier, Version};
use diesel::{r2d2::ConnectionManager, ExpressionMethods, PgConnection, QueryDsl, RunQueryDsl};
use r2d2::Pool;
use secrecy::{ExposeSecret, Secret};
use uuid::Uuid;

use crate::models::VerificationInfo;



#[derive(thiserror::Error, Debug)] 
pub enum AuthError {
    #[error("Invalid credentials.")] 
    InvalidCredentials(#[source] anyhow::Error),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}


pub struct Credentials {
    pub username: String,
    pub password: Secret<String>,
}


#[tracing::instrument(name = "Validate credentials", skip(credentials, pool))]
pub async fn validate_credentials(
    credentials: Credentials,
    pool: &Pool<ConnectionManager<PgConnection>>,
) -> Result<uuid::Uuid, AuthError>{
    let mut user_id: Option<Uuid> = None;
    let mut expected_password_hash = Secret::new(
        "$argon2id$v=19$m=15000,t=2,p=1$\
        gZiV/M1gPc22ElAH/Jh1Hw$\
        CWOrkoo7oJBQ/iyh7uJ0LO2aLEfrHwTWllSAxT0zRno"
            .to_string()
    );

    if let Some((stored_password_hash, stored_user_id))
        = get_stored_credentials(&credentials.username, &pool)
            .await?
    {
        user_id = Some(stored_user_id);
        expected_password_hash = stored_password_hash;
    }

    let current_span = tracing::Span::current();
    tokio::task::spawn_blocking(move || {
        current_span.in_scope(|| {
            verify_password_hash(
                expected_password_hash,
                credentials.password
            )
        })
    })
    .await
    .context("Failed to spawn blocking task.")??;

    user_id.ok_or_else(
        || AuthError::InvalidCredentials(anyhow::anyhow!("Unknown username."))
    )
}

#[tracing::instrument(
    name = "Set new password",
    skip(uid, password, pool)
)]
pub async fn change_password(
    uid: Uuid,
    password: Secret<String>,
    pool: &Pool<ConnectionManager<PgConnection>>
) -> Result<(), anyhow::Error> {
    let current_span = tracing::Span::current();
    let password_hash = web::block(move || {
        current_span.in_scope(|| {
            compute_password_hash(password)
                .context("Failed to compute new password hash")
        })
    })
    .await
    .context("Failed due to threadpool error")??;

    {
        use crate::schema::users::dsl::*;
        use diesel::prelude::*;
        use diesel::update;

        let mut conn = pool.get().context("Failed to get DB connection from pool")?;
        let current_span = tracing::Span::current();

        web::block(move || current_span.in_scope(move || {
            update(users.filter(user_id.eq(uid)))
                .set(password.eq(password_hash.expose_secret()))
                .execute(&mut conn)
                .context("Failed to update password in database")
        })).await
        .context("Failed due to threadpool error")??;

    }

    Ok(())
}

#[tracing::instrument(
    name = "Verify password hash",
    skip(expected_password_hash, password_candidate)
)]
pub fn verify_password_hash(
    expected_password_hash: Secret<String>,
    password_candidate: Secret<String>,
) -> Result<(), AuthError> {
        let expected_password_hash = PasswordHash::new(
            expected_password_hash.expose_secret()
            )
            .context("Failed to parse hash in PHC string format.")?;

        Argon2::default()
            .verify_password(
                password_candidate.expose_secret().as_bytes(),
                &expected_password_hash
            )
            .context("Invalid password.") 
            .map_err(AuthError::InvalidCredentials)
}

#[tracing::instrument(name = "Get stored credentials", skip(uname, pool))]
pub async fn get_stored_credentials(uname: &str, pool: &Pool<ConnectionManager<PgConnection>>) -> Result<Option<(Secret<String>, Uuid)>, AuthError> {
    use crate::schema::users::dsl::*;

    let mut conn = pool.get().context("Failed to get connection from pool")?;
    let uname = uname.to_string();
    
    let result = web::block(move || {

        let row: Result<VerificationInfo, AuthError> = users.select((user_id, password))
            .limit(1)
            .filter(username.eq(uname))
            .first::<VerificationInfo>(&mut conn)
            .context("Failed to query user")
            .map_err(AuthError::InvalidCredentials);

        row
    })
    .await
    .context("Failed due threadpool error")
    .map_err(AuthError::UnexpectedError)??;


    Ok(Some((Secret::new(result.password), result.user_id)))
}

fn compute_password_hash(
    password: Secret<String>
) -> Result<Secret<String>, anyhow::Error> {
        let salt = SaltString::generate(&mut rand::thread_rng());
        let password_hash = Argon2::new(
            Algorithm::Argon2id,
            Version::V0x13,
            Params::new(15000, 2, 1, None).unwrap(),
        )
        .hash_password(password.expose_secret().as_bytes(), &salt)?
        .to_string();

        Ok(Secret::new(password_hash))
}
