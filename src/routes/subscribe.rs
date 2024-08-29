use std::error::Error;

use actix_web::{error::BlockingError, web, HttpResponse};
use chrono::Utc;
use diesel::{r2d2::{ConnectionManager, Pool}, PgConnection, QueryResult, RunQueryDsl};
use uuid::Uuid;

use crate::models::{Subscription, SubscriptionAdd};
use crate::schema::subscriptions::dsl::*;

#[tracing::instrument(
    name = "Adding a new subscriber",
    skip(form, pool),
    fields(
        subscriber_email = %form.email,
        subscriber_name= %form.name
    ) 
)]
pub async fn subscribe(form: web::Form<Subscription>, pool: web::Data<Pool<ConnectionManager<PgConnection>>>) -> HttpResponse {
    let insert = SubscriptionAdd{
        id: Uuid::new_v4(),
        email: form.email.clone(),
        name: form.name.clone(),
        subscribed_at: Utc::now()
    };

    let result = insert_subscriber(&pool, insert);
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
    insert: SubscriptionAdd
) -> Result<QueryResult<usize>, Box<dyn Error>> {
    let mut conn = pool.get()?;
    web::block(move || {
        diesel::insert_into(subscriptions)
        .values(insert)
        .execute(&mut conn)
    })
    .await.map_err(|_| "Failed insertion into DB".into())
}
