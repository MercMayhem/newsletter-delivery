use actix_web::web;
use anyhow::Context;
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use diesel::{r2d2::ConnectionManager, ExpressionMethods, PgConnection, QueryDsl, RunQueryDsl};
use r2d2::Pool;
use secrecy::{ExposeSecret, Secret};
use uuid::Uuid;

use crate::{models::VerificationInfo, routes::newsletter_delivery::PublishError};



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
name = "Verify password hash", skip(expected_password_hash, password_candidate)
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
