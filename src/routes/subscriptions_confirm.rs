use actix_web::{web, HttpResponse};
use diesel::{r2d2::{ConnectionManager, Pool}, ExpressionMethods, PgConnection, QueryDsl, RunQueryDsl};
use serde::Deserialize;
use uuid::Uuid;

use crate::{models::SubscriptionToken, schema::subscription_tokens::dsl::*};
use crate::schema::subscriptions;

#[derive(Deserialize)]
pub struct Parameters{
    subscription_token: String
}

#[tracing::instrument(
    name = "Confirm a pending subscriber",
    skip(parameters)
)]
pub async fn confirm(parameters: web::Query<Parameters>, pool: web::Data<Pool<ConnectionManager<PgConnection>>>) -> HttpResponse{
    let mut conn = pool.get().unwrap();
    
    let result = web::block(move || {
        subscription_tokens
            .filter(subscription_token.eq(parameters.subscription_token.clone()))
            .first::<SubscriptionToken>(&mut conn)
            
    }).await.unwrap();

    let id: Uuid = match result{
        Ok(saved) => {
            saved.subscriber_id
        },

        Err(_) => {
            return HttpResponse::InternalServerError().finish()
        }
    };

    let mut conn = pool.get().unwrap();
    let result = web::block(move || {
        diesel::update(subscriptions::dsl::subscriptions)
            .filter(subscriptions::dsl::id.eq(id))
            .set(subscriptions::dsl::status.eq("confirmed"))
            .execute(&mut conn)
    }).await.unwrap();
    
    match result{
        Ok(_) => {
            HttpResponse::Ok().finish()
        },

        Err(_) => {
            HttpResponse::InternalServerError().finish()
        }
    }
}
