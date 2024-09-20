use actix_web::{http::StatusCode, web, HttpRequest, HttpResponse, ResponseError};
use actix_web_flash_messages::FlashMessage;
use anyhow::Context;
use chrono::Utc;
use r2d2::PooledConnection;
use serde::Deserialize;
use diesel::{r2d2::{ConnectionManager, Pool}, PgConnection};
use diesel::prelude::*;
use uuid::Uuid;

use crate::{email_client::EmailClient, idempotency::{get_saved_response, persistence::{save_response, try_processing, NextAction}, IdempotencyKey}, models::{IssueDeliveryQueue, NewsletterIssue, Subscription}, routes::admin::dashboard::get_username, session_state::UserId, utils::see_other};
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
    html: String,
    idempotency_key: String
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

    let BodyData{ title, text, html, idempotency_key } = body.0;
    let idempotency_key: IdempotencyKey = idempotency_key.try_into().map_err(PublishError::UnexpectedError)?;


    let user_id = user_id.into_inner();
    tracing::Span::current().record("user_id", &tracing::field::display(&*user_id));

    let username = get_username(*user_id, &pool).await.map_err(PublishError::UnexpectedError)?;
    tracing::Span::current().record("username", &tracing::field::display(username));

    match try_processing(&pool, &idempotency_key, *user_id).await.map_err(PublishError::UnexpectedError)?{
        NextAction::StartProcessing => {},
        NextAction::ReturnSavedResponse(saved_response) => {
            FlashMessage::info("Successfully sent newsletter.").send();
            return Ok(saved_response);
        }
    }

    insert_issue_and_enqueue_tasks(
        &pool,
        title,
        text,
        html
    )
    .await?;

    FlashMessage::info("Successfully sent newsletter.").send();
    let response = see_other("/admin/newsletter");
    let response = save_response(&pool, &idempotency_key, *user_id, response)
                    .await
                    .map_err(PublishError::UnexpectedError)?;

    Ok(response)
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

#[tracing::instrument(skip_all)] 
pub async fn insert_issue_and_enqueue_tasks(
    pool: &Pool<ConnectionManager<PgConnection>>,
    title_val: String,
    text_content: String,
    html_content: String,
) -> Result<(), anyhow::Error> {
    let mut conn = pool.get()?;

    let current_span = tracing::Span::current();

    web::block(move || {
        current_span.in_scope(|| {
            conn.transaction::<_, anyhow::Error, _>(|conn| {
                let newsletter_issue_id = insert_newsletter_issue(conn, title_val, text_content, html_content)
                    .context("Failed to store newsletter issue details")?;

                enqueue_delivery_tasks(conn, newsletter_issue_id)
                    .context("Failed to enqueue delivery tasks")?;
                
                Ok(())
            })
            .context("Failed to execute insertion of issue and enqueuing of tasks")
        })
    })
    .await
    .context("Failed due to threadpool error")??;

    Ok(())
}

#[tracing::instrument(skip_all)] 
fn insert_newsletter_issue(
    conn: &mut PooledConnection<ConnectionManager<PgConnection>>,
    title_val: String,
    text_content: String,
    html_content: String,
) -> Result<Uuid, anyhow::Error>{
    use diesel::prelude::*;
    use crate::schema::newsletter_issues::dsl::*;

    let newsletter_issue_id_val = Uuid::new_v4();
    let issue = NewsletterIssue{
        newsletter_issue_id: newsletter_issue_id_val,
        title: title_val.clone(),
        text: text_content.clone(),
        html: html_content.clone(),
        published_at: Utc::now().to_string()
    };

    diesel::insert_into(newsletter_issues)
        .values(&issue)
        .execute(conn)?;

    Ok(newsletter_issue_id_val)
}

#[tracing::instrument(skip_all)] 
fn enqueue_delivery_tasks(
    conn: &mut PooledConnection<ConnectionManager<PgConnection>>,
    newsletter_issue_id_val: Uuid
) -> Result<(), anyhow::Error> {
    use diesel::prelude::*;

    let confirmed_emails: Vec<String> = {
        use crate::schema::subscriptions::dsl::*;

        subscriptions.filter(status.eq("confirmed"))
            .select(email)
            .load(conn)?
    };

    let new_entries: Vec<IssueDeliveryQueue> = confirmed_emails
        .iter()
        .map(|subscriber_email| IssueDeliveryQueue {
            newsletter_issue_id: newsletter_issue_id_val,
            subscriber_email: subscriber_email.to_string(),
        })
        .collect();

    {
        use crate::schema::issue_delivery_queue::dsl::*;

        diesel::insert_into(issue_delivery_queue)
            .values(&new_entries)
            .execute(conn)?;
    }

    Ok(())
}
