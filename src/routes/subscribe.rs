use std::{error::Error, ops::SubAssign};

use actix_web::{web, HttpResponse};
use chrono::Utc;
use diesel::{r2d2::{ConnectionManager, Pool}, PgConnection, QueryResult, RunQueryDsl};
use uuid::Uuid;

use crate::{domain::{new_subscriber::NewSubscriber, subscriber_email::SubscriberEmail, subscriber_name::SubscriberName}, models::{Subscription, SubscriptionAdd}};
use crate::schema::subscriptions::dsl::*;

impl TryFrom<Subscription> for NewSubscriber {
    type Error = String;
    fn try_from(value: Subscription) -> Result<Self, Self::Error> {
        let sub_name = SubscriberName::parse(value.name)?;
        let sub_email = SubscriberEmail::parse(value.email)?;

        Ok(Self{ name: sub_name, email: sub_email })
    }
}

#[tracing::instrument(
    name = "Adding a new subscriber",
    skip(form, pool),
    fields(
        subscriber_email = %form.email,
        subscriber_name= %form.name
    ) 
)]
pub async fn subscribe(form: web::Form<Subscription>, pool: web::Data<Pool<ConnectionManager<PgConnection>>>) -> HttpResponse {
    let new_subscriber: NewSubscriber = match form.0.try_into() {
        Ok(form) => form,
        Err(e) => return HttpResponse::BadRequest().body(e)
    };

    let result = insert_subscriber(&pool, new_subscriber);
    if let Ok(res) = result.await {
        match res{
            Ok(_) => {
                tracing::info!("New subscriber has been saved successfully.");
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
    insert: NewSubscriber
) -> Result<QueryResult<usize>, Box<dyn Error>> {
    let insert = SubscriptionAdd{
        id: Uuid::new_v4(),
        email: insert.email.inner(),
        name: insert.name.inner(),
        subscribed_at: Utc::now()
    };

    let mut conn = pool.get()?;
    web::block(move || {
        diesel::insert_into(subscriptions)
        .values(insert)
        .execute(&mut conn)
    })
    .await.map_err(|_| "Failed insertion into DB".into())
}
