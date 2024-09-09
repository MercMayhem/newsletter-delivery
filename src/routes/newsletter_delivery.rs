use actix_web::{http::StatusCode, web, HttpResponse, ResponseError};
use anyhow::Context;
use serde::Deserialize;
use diesel::{r2d2::{ConnectionManager, Pool}, PgConnection};
use diesel::prelude::*;

use crate::{email_client::EmailClient, models::Subscription};
use crate::domain::subscriber_email::SubscriberEmail;

use super::subscribe::error_chain_fmt;

#[derive(thiserror::Error)]
pub enum PublishError {
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for PublishError {
fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for PublishError {
    fn status_code(&self) -> StatusCode{
        match self {
        PublishError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
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
    skip(body, pool, email_client)
)]
pub async fn newsletter_delivery(body: web::Json<BodyData>, pool: web::Data<Pool<ConnectionManager<PgConnection>>>, email_client: web::Data<EmailClient>) -> Result<HttpResponse, PublishError>{
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
