use std::error::Error;

use actix_web::{web, HttpResponse};
use chrono::Utc;
use diesel::{associations::HasTable, r2d2::{ConnectionManager, Pool}, Connection, PgConnection, RunQueryDsl};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use uuid::Uuid;

use crate::{domain::{new_subscriber::NewSubscriber, subscriber_email::SubscriberEmail, subscriber_name::SubscriberName}, email_client::EmailClient, models::{SubscribeFormData, SubscriptionAdd, SubscriptionTokensAdd}, startup::ApplicationBaseUrl};
use crate::schema::subscriptions::dsl::*;
use crate::schema::subscription_tokens::dsl::*;

impl TryFrom<SubscribeFormData> for NewSubscriber {
    type Error = String;
    fn try_from(value: SubscribeFormData) -> Result<Self, Self::Error> {
        let sub_name = SubscriberName::parse(value.name)?;
        let sub_email = SubscriberEmail::parse(value.email)?;

        Ok(Self{ name: sub_name, email: sub_email })
    }
}

#[tracing::instrument(
    name = "Adding a new subscriber",
    skip(form, pool, email_client, base_url),
    fields(
        subscriber_email = %form.email,
        subscriber_name= %form.name
    ) 
)]
pub async fn subscribe(form: web::Form<SubscribeFormData>, pool: web::Data<Pool<ConnectionManager<PgConnection>>>, email_client: web::Data<EmailClient>, base_url: web::Data<ApplicationBaseUrl>) -> HttpResponse {
    let new_subscriber: NewSubscriber = match form.0.try_into() {
        Ok(form) => form,
        Err(e) => return HttpResponse::BadRequest().body(e)
    };

    let insert_op = insert_subscriber(&pool, &new_subscriber).await;
    match insert_op {
        Ok(subscriber_token) => {
            tracing::info!("New subscriber has been saved successfully.");

            if send_confirmation_mail(&email_client, new_subscriber, &base_url.0, &subscriber_token).await.is_err(){
                return HttpResponse::InternalServerError().finish();
            }

            HttpResponse::Ok().finish()
        },

        Err(_) => {
            tracing::error!("Failed to execute query");
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[tracing::instrument(
    name = "Saving new subscriber details in the database",
    skip(insert, pool)
)]
pub async fn insert_subscriber(
    pool: &Pool<ConnectionManager<PgConnection>>,
    insert: &NewSubscriber
) -> Result<String, Box<dyn Error>> {
    let sub_id = Uuid::new_v4();
    let sub_token = generate_subscription_token();

    let insert_subscriptions = SubscriptionAdd{
        id: sub_id,
        email: insert.email.inner(),
        name: insert.name.inner(),
        subscribed_at: Utc::now(),
        status: "pending_confirmation".into()
    };

    let insert_subscription_token = SubscriptionTokensAdd{
        subscriber_id: sub_id,
        subscription_token: sub_token.clone()
    };

    let mut conn = pool.get()?;

    let result = web::block(move || {
        conn.transaction(|conn|{
            diesel::insert_into(subscriptions::table())
            .values(insert_subscriptions)
            .execute(conn)?;

            diesel::insert_into(subscription_tokens)
                .values(insert_subscription_token)
                .execute(conn)?;

            diesel::result::QueryResult::Ok(())
        })
    }).await;

    if result.is_err() {
        return Err("Failed due to blocking error".into());
    }

    result.unwrap() 
        .map_err(|_| "Failed insertion into DB".to_string())?;

    Ok(sub_token)
}

#[tracing::instrument(
    name = "Sending confirmation mail to subscriber",
    skip(email_client, new_subscriber, base_url)
)]
pub async fn send_confirmation_mail(email_client: &EmailClient, new_subscriber: NewSubscriber, base_url: &String, sub_token: &str) -> Result<(), reqwest::Error>{
    let confirmation_link = format!("{}/subscriptions/confirm?subscription_token={}", base_url, sub_token);

    let res = email_client.send_email(new_subscriber.email,
        "Welcome!",
        &format!("Welcome to our newsletter! Click <a href = \"{}\">here</a> to confirm your subscription", confirmation_link),
        &format!("Welcome to our newsletter! Visit {} to confirm subscription", confirmation_link)
    ).await;

    if res.is_err(){
        tracing::error!("Couldn't send email");
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
