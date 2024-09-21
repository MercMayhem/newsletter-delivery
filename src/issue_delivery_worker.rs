use std::time::Duration;

use actix_web::web;
use diesel::{r2d2::ConnectionManager, Connection, PgConnection};
use r2d2::{Pool, PooledConnection};
use uuid::Uuid;

use crate::{block_email_client::BlockEmailClient, configuration::Settings, domain::subscriber_email::SubscriberEmail, models::{IssueDeliveryQueue, NewsletterIssue}, startup::get_connection_pool};

pub async fn run_worker_until_stopped(
    configuration: Settings
) -> Result<(), anyhow::Error>{
    let connection_pool = get_connection_pool(&configuration.database);
    let email_client = configuration.email_client.blocking_client();

    worker_loop(connection_pool, email_client).await
}

async fn worker_loop(pool: Pool<ConnectionManager<PgConnection>>, email_client: BlockEmailClient) -> Result<(), anyhow::Error>{

    loop{
        let mut conn = pool.get()?;
        let current_span = tracing::Span::current();
        let client_clone = email_client.clone();

        let transaction = web::block(move ||{
            current_span.in_scope(||{
                conn.transaction(|conn| {
                    try_execute_task(conn, &client_clone)
                })
            })
        })
        .await?;

        match transaction{
            Ok(ExecutionOutcome::EmptyQueue) => {
                tokio::time::sleep(Duration::from_secs(10)).await;
            },

            Err(_) => {
                tokio::time::sleep(Duration::from_secs(1)).await;
            },

            Ok(ExecutionOutcome::TaskCompleted) => {}
        }
    }
}

pub enum ExecutionOutcome{
    TaskCompleted,
    EmptyQueue
}

#[tracing::instrument(
    skip_all,
    fields(
        newsletter_issue_id=tracing::field::Empty,
        subscriber_email=tracing::field::Empty
    )
)]
pub fn try_execute_task(
    conn: &mut PooledConnection<ConnectionManager<PgConnection>>,
    email_client: &BlockEmailClient
) -> Result<ExecutionOutcome, anyhow::Error> {
    let task = dequeue_task(conn)?;
    if task.is_none(){
        return Ok(ExecutionOutcome::EmptyQueue);
    }
    

    if let Some((issue_id, email)) = task{
        tracing::Span::current()
            .record("newsletter_issue_id", tracing::field::display(issue_id))
            .record("subscriber_email", tracing::field::display(email.clone()));

        match SubscriberEmail::parse(email.clone()){
            Ok(email) => {
                let issue = get_issue(conn, issue_id)?;
                if let Err(e) = email_client
                    .send_email(
                        &email,
                        &issue.title,
                        &issue.html,
                        &issue.text,
                    )
                {
                    tracing::error!(
                        error.cause_chain = ?e,
                        error.message = %e,
                        "Failed to deliver issue to a confirmed subscriber. \
                         Skipping.",
                    );
                }
            },

            Err(e) => {
                tracing::error!(
                    error.cause_chain = ?e,
                    error.message = %e,
                    "Skipping a confirmed subscriber. \
                     Their stored contact details are invalid",
                );
            }
        }

        delete_task(conn, issue_id, &email)?;
    }

    Ok(ExecutionOutcome::TaskCompleted)
}

fn get_issue(
    conn: &mut PooledConnection<ConnectionManager<PgConnection>>,
    issue_id_val: Uuid
) -> Result<NewsletterIssue, anyhow::Error>{
    use diesel::prelude::*;
    use crate::schema::newsletter_issues::dsl::*;

    Ok(newsletter_issues
        .select((newsletter_issue_id, title, text, html, published_at))
        .filter(newsletter_issue_id.eq(issue_id_val))
        .first::<NewsletterIssue>(conn)?)
}

#[tracing::instrument(skip_all)]
fn dequeue_task(conn: &mut PooledConnection<ConnectionManager<PgConnection>>) -> Result<Option<(Uuid, String)>, anyhow::Error>{
    use diesel::prelude::*;
    use crate::schema::issue_delivery_queue::dsl::*;

    let r: Option<IssueDeliveryQueue> = issue_delivery_queue
        .select((newsletter_issue_id, subscriber_email))
        .for_update()
        .skip_locked()
        .limit(1)
        .first::<IssueDeliveryQueue>(conn)
        .optional()?;

    if let Some(r) = r{
        Ok(Some((
            r.newsletter_issue_id,
            r.subscriber_email
        )))
    } else {
        Ok(None)
    }
}

#[tracing::instrument(skip_all)]
fn delete_task(conn: &mut PooledConnection<ConnectionManager<PgConnection>>,issue_id: Uuid, email: &str) -> Result<(), anyhow::Error>{
    use diesel::prelude::*;
    use crate::schema::issue_delivery_queue::dsl::*;

    diesel::delete(
        issue_delivery_queue
            .filter(newsletter_issue_id.eq(issue_id))
            .filter(subscriber_email.eq(email))
    )
    .execute(conn)?;

    Ok(())
}

