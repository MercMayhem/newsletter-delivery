use std::error::Error;

use actix_web::{web, HttpResponse};
use chrono::Utc;
use diesel::{r2d2::{ConnectionManager, Pool}, PgConnection, QueryResult, RunQueryDsl};
use uuid::Uuid;

use crate::{domain::{new_subscriber::NewSubscriber, subscriber_email::SubscriberEmail, subscriber_name::SubscriberName}, email_client::EmailClient, models::{SubscribeFormData, SubscriptionAdd}};
use crate::schema::subscriptions::dsl::*;

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
    skip(form, pool, email_client),
    fields(
        subscriber_email = %form.email,
        subscriber_name= %form.name
    ) 
)]
pub async fn subscribe(form: web::Form<SubscribeFormData>, pool: web::Data<Pool<ConnectionManager<PgConnection>>>, email_client: web::Data<EmailClient>) -> HttpResponse {
    let new_subscriber: NewSubscriber = match form.0.try_into() {
        Ok(form) => form,
        Err(e) => return HttpResponse::BadRequest().body(e)
    };

    
    if let Ok(res) = insert_subscriber(&pool, &new_subscriber).await {
        match res{
            Ok(_) => {
                tracing::info!("New subscriber has been saved successfully.");
                if send_confirmation_mail(&email_client, new_subscriber).await.is_err(){
                    return HttpResponse::InternalServerError().finish();
                }
                HttpResponse::Ok().finish()
            },
            Err(e) => {
                tracing::error!("Failed to execute query: {:?}", e);
                HttpResponse::InternalServerError().finish()
            }
        }
    } else {
        tracing::error!("Failed to execute query");
        HttpResponse::InternalServerError().finish()
    }
}

#[tracing::instrument(
    name = "Saving new subscriber details in the database",
    skip(insert, pool)
)]
pub async fn insert_subscriber(
    pool: &Pool<ConnectionManager<PgConnection>>,
    insert: &NewSubscriber
) -> Result<QueryResult<usize>, Box<dyn Error>> {
    let insert = SubscriptionAdd{
        id: Uuid::new_v4(),
        email: insert.email.inner(),
        name: insert.name.inner(),
        subscribed_at: Utc::now(),
        status: "pending_confirmation".into()
    };

    let mut conn = pool.get()?;
    web::block(move || {
        diesel::insert_into(subscriptions)
        .values(insert)
        .execute(&mut conn)
    })
    .await.map_err(|_| "Failed insertion into DB".into())
}

#[tracing::instrument(
    name = "Sending confirmation mail to subscriber",
    skip(email_client, new_subscriber)
)]
pub async fn send_confirmation_mail(email_client: &EmailClient, new_subscriber: NewSubscriber) -> Result<(), reqwest::Error>{
    let confirmation_link = "https://my-api.com/subscriptions/confirm";

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
