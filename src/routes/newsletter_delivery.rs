use actix_web::{http::{header::{HeaderMap, HeaderValue}, StatusCode}, web, HttpRequest, HttpResponse, ResponseError};
use anyhow::Context;
use secrecy::Secret;
use serde::Deserialize;
use diesel::{r2d2::{ConnectionManager, Pool}, PgConnection};
use diesel::prelude::*;
use base64::prelude::*;

use crate::{authentication::{validate_credentials, AuthError, Credentials}, email_client::EmailClient, models::Subscription};
use crate::domain::subscriber_email::SubscriberEmail;

use super::subscribe::error_chain_fmt;

#[derive(thiserror::Error)]
pub enum PublishError {
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
    #[error("Authentication Failed")]
    AuthError(#[source] anyhow::Error),
}

impl std::fmt::Debug for PublishError {
fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for PublishError {
    fn error_response(&self) -> HttpResponse {
        match self { 
            PublishError::UnexpectedError(_) => {
                HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR)
            }
            PublishError::AuthError(_) => {
                let mut response = HttpResponse::new(StatusCode::UNAUTHORIZED);
                let header_value = HeaderValue::from_str(r#"Basic realm="publish""#)
                    .unwrap();
                response
                    .headers_mut()
                    .insert(actix_web::http::header::WWW_AUTHENTICATE, header_value);
                
                response
            } 
        }
    }
}

#[derive(Deserialize)]
pub struct BodyData{
    title: String,
    content: Content
}

#[derive(Deserialize)]
pub struct Content{
    text: String,
    html: String
}

pub struct ConfirmedSubscriber{
    email: SubscriberEmail
}

#[tracing::instrument(
    name = "Sending newsletter to confirmed subscribers",
    skip(body, pool, email_client),
    fields(username=tracing::field::Empty, user_id=tracing::field::Empty)
)]
pub async fn newsletter_delivery(body: web::Json<BodyData>, pool: web::Data<Pool<ConnectionManager<PgConnection>>>, email_client: web::Data<EmailClient>, request: HttpRequest) -> Result<HttpResponse, PublishError>{
    let credentials = basic_authentication(request.headers())
        .map_err(PublishError::AuthError)?;

    tracing::Span::current().record(
        "username",
        &tracing::field::display(&credentials.username)
    );

    let user_id = validate_credentials(credentials, &pool)
                    .await
                    .map_err(|e| match e{
                        AuthError::InvalidCredentials(_) => PublishError::AuthError(e.into()),
                        AuthError::UnexpectedError(_) => PublishError::UnexpectedError(e.into())
                    })?;
    tracing::Span::current().record("user_id", &tracing::field::display(&user_id));

    let subscriptions = get_confirmed_subscribers(&pool).await?;

    
    for subscription in subscriptions {
        match subscription {
            Ok(subscriber) => {
                email_client
                    .send_email(
                        &subscriber.email,
                        &body.title,
                        &body.content.html,
                        &body.content.text
                    )
                    .await
                    .with_context(|| {
                            format!(
                                "Failed to send newsletter issue to {}",
                                subscriber.email
                            )
                        }
                    )?
            },

            Err(error) => {
                tracing::warn!(
                    error.cause_chain = ?error,
                    "Skipping a confirmed subscriber. \
                    Their stored contact details are invalid",
                )
            }
        }
    }

    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(
    name = "Get confirmed subscribers",
    skip(pool)
)]
async fn get_confirmed_subscribers(pool: &Pool<ConnectionManager<PgConnection>>) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error>{
    use crate::schema::subscriptions::dsl::*;
    
    let mut conn = pool.get()?;
    let subscription_vec: Vec<Subscription> = subscriptions
                        .select((email, name, status))
                        .filter(status.eq("confirmed"))
                        .load::<Subscription>(&mut conn)?;

    Ok(
        subscription_vec
            .iter()
            .map(|elem| {
                match SubscriberEmail::parse(elem.email.clone()){
                    Ok(e) => Ok(ConfirmedSubscriber{email: e}),
                    Err(error) => Err(anyhow::anyhow!(error))
                }
            })
            .collect()
    )
}


fn basic_authentication(headers: &HeaderMap) -> Result<Credentials, anyhow::Error> {
    let header_value = headers
        .get("Authorization")
        .context("The 'Authorization' header was missing")?
        .to_str()
        .context("The 'Authorization' was not a valid UTF8 string")?;

    let base64encoded_segment = header_value
        .strip_prefix("Basic ")
        .context("The authorization scheme was not 'Basic'.")?;
    
    let decoded_bytes = BASE64_STANDARD.decode(base64encoded_segment)
        .context("Failed to base64-decode 'Basic' credentials")?;

    let decoded_credentials = String::from_utf8(decoded_bytes)
        .context("The decoded credential string is not valid UTF8.")?;

    let mut credentials = decoded_credentials.splitn(2, ':');
    let username = credentials
        .next()
        .ok_or_else(|| anyhow::anyhow!("A username must be provided in 'Basic' auth."))?
        .to_string();
    let password = credentials
        .next()
        .ok_or_else(|| anyhow::anyhow!("A password must be provided in 'Basic' auth."))? 
        .to_string();

    Ok(Credentials{ 
        username,
        password: Secret::new(password) 
    })
}



