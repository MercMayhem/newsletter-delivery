
use actix_web::{error::BlockingError, http::StatusCode, web, HttpResponse, ResponseError};
use chrono::Utc;
use diesel::{
    associations::HasTable, r2d2::{ConnectionManager, Pool}, Connection, PgConnection, RunQueryDsl
};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use uuid::Uuid;

use crate::{schema::subscription_tokens::dsl::*, traits::SubscriptionService};
use crate::schema::subscriptions::dsl::*;
use crate::{
    domain::{
        new_subscriber::NewSubscriber, subscriber_email::SubscriberEmail,
        subscriber_name::SubscriberName,
    },
    email_client::EmailClient,
    models::{SubscribeFormData, SubscriptionAdd, SubscriptionTokensAdd},
    startup::ApplicationBaseUrl,
};

impl TryFrom<SubscribeFormData> for NewSubscriber {
    type Error = String;
    fn try_from(value: SubscribeFormData) -> Result<Self, Self::Error> {
        let sub_name = SubscriberName::parse(value.name)?;
        let sub_email = SubscriberEmail::parse(value.email)?;

        Ok(Self {
            name: sub_name,
            email: sub_email,
        })
    }
}

#[derive(thiserror::Error)]
pub enum SubscribeError{
    #[error("{0}")]
    ValidationError(String),
    #[error("Failed to insert subscriber to database")]
    InsertSubscriberError(#[from] InsertSubscriberError),
    #[error("Failed to send confirmation email to user")]
    SendEmailError(#[from] reqwest::Error)
}

impl std::fmt::Debug for SubscribeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for SubscribeError{
    fn status_code(&self) -> actix_web::http::StatusCode {
        match self {
            Self::ValidationError(_) => StatusCode::BAD_REQUEST,
            Self::InsertSubscriberError(_) | Self::SendEmailError(_) => StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

#[tracing::instrument(
    name = "Adding a new subscriber",
    skip(form, subscription_service),
    fields(
        subscriber_email = %form.email,
        subscriber_name= %form.name
    ) 
)]
pub async fn subscribe<S: SubscriptionService>(
    form: web::Form<SubscribeFormData>,
    // pool: web::Data<Pool<ConnectionManager<PgConnection>>>,
    // email_client: web::Data<EmailClient>,
    // base_url: web::Data<ApplicationBaseUrl>,
    subscription_service: web::Data<S>,
) -> Result<HttpResponse, SubscribeError>{
    subscription_service
        .create_subscription(form.0)
        .await?;

    Ok(HttpResponse::Ok().finish())
}

#[derive(thiserror::Error)]
#[error("Error Inserting Subscriber to DB")]
pub enum InsertSubscriberError{
    DbPoolErr(#[from] r2d2::Error),
    TransactionError(#[from] diesel::result::Error),
    ThreadPoolErr(#[from] BlockingError)
}

pub fn error_chain_fmt(e: &impl std::error::Error, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}\n", e)?;
    let mut current = e.source();

    while let Some(cause) = current {
        write!(f, "\nCaused By:\n\t{}", cause)?;
        current = cause.source();
    }

    Ok(())
}

impl std::fmt::Debug for InsertSubscriberError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

#[tracing::instrument(
    name = "Saving new subscriber details in the database",
    skip(insert, pool)
)]
pub async fn insert_subscriber(
    pool: &Pool<ConnectionManager<PgConnection>>,
    insert: &NewSubscriber,
) -> Result<String, InsertSubscriberError> {
    let sub_id = Uuid::new_v4();
    let sub_token = generate_subscription_token();

    let insert_subscriptions = SubscriptionAdd {
        id: sub_id,
        email: insert.email.inner(),
        name: insert.name.inner(),
        subscribed_at: Utc::now(),
        status: "pending_confirmation".into(),
    };

    let insert_subscription_token = SubscriptionTokensAdd {
        subscriber_id: sub_id,
        subscription_token: sub_token.clone(),
    };

    let mut conn = pool.get().map_err(|err| {
        InsertSubscriberError::DbPoolErr(err)
    })?;


    let thread_result = web::block(move || {
        conn.transaction(|conn| {
            diesel::insert_into(subscriptions::table())
                .values(insert_subscriptions)
                .execute(conn)?;

            diesel::insert_into(subscription_tokens)
                .values(insert_subscription_token)
                .execute(conn)?;

            diesel::result::QueryResult::Ok(())
        })
    })
    .await;

    match thread_result {
        Ok(r) => {
            if let Err(e) = r {
                return Err(InsertSubscriberError::TransactionError(e));
            }
            return Ok(sub_token);
        },

        Err(e) => {
            return Err(InsertSubscriberError::ThreadPoolErr(e));
        }
    }

}

#[tracing::instrument(
    name = "Sending confirmation mail to subscriber",
    skip(email_client, new_subscriber, base_url)
)]
pub async fn send_confirmation_mail(
    email_client: &EmailClient,
    new_subscriber: &NewSubscriber,
    base_url: &String,
    sub_token: &str,
) -> Result<(), reqwest::Error> {
    let confirmation_link = format!(
        "{}/subscriptions/confirm?subscription_token={}",
        base_url, sub_token
    );

    let res = email_client.send_email(&new_subscriber.email,
        "Welcome!",
        &format!("Welcome to our newsletter! Click <a href = \"{}\">here</a> to confirm your subscription", confirmation_link),
        &format!("Welcome to our newsletter! Visit {} to confirm subscription", confirmation_link)
    ).await;

    if res.is_err() {
        return res;
    }

    Ok(())
}

fn generate_subscription_token() -> String {
    let mut rng = thread_rng();
    std::iter::repeat_with(|| rng.sample(Alphanumeric))
        .map(char::from)
        .take(25)
        .collect()
}
