use actix_web::{http::StatusCode, web, HttpRequest, HttpResponse, ResponseError};
use actix_web_flash_messages::FlashMessage;
use anyhow::Context;
use serde::Deserialize;
use diesel::{r2d2::{ConnectionManager, Pool}, PgConnection};
use diesel::prelude::*;

use crate::{email_client::EmailClient, models::Subscription, routes::admin::dashboard::get_username, session_state::UserId, utils::see_other};
use crate::domain::subscriber_email::SubscriberEmail;

use crate::routes::subscribe::error_chain_fmt;

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
    fn error_response(&self) -> HttpResponse {
        match self { 
            PublishError::UnexpectedError(_) => {
                HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct BodyData{
    title: String,
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
pub async fn newsletter_delivery(body: web::Form<BodyData>, pool: web::Data<Pool<ConnectionManager<PgConnection>>>, email_client: web::Data<EmailClient>, request: HttpRequest, user_id: web::ReqData<UserId>) -> Result<HttpResponse, PublishError>{

    let user_id = user_id.into_inner();
    tracing::Span::current().record("user_id", &tracing::field::display(&*user_id));

    let username = get_username(*user_id, &pool).await.map_err(PublishError::UnexpectedError)?;
    tracing::Span::current().record("username", &tracing::field::display(username));

    let subscriptions = get_confirmed_subscribers(&pool).await?;
    
    for subscription in subscriptions {
        match subscription {
            Ok(subscriber) => {
                email_client
                    .send_email(
                        &subscriber.email,
                        &body.title,
                        &body.html,
                        &body.text
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


    FlashMessage::info("Successfully sent newsletter.").send();
    Ok(see_other("/admin/newsletter"))
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


