use actix_web::{web, HttpResponse};
use diesel::{
    r2d2::{ConnectionManager, Pool},
    ExpressionMethods, PgConnection, QueryDsl, RunQueryDsl,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{schema::subscriptions, traits::SubscriptionService};
use crate::{models::SubscriptionToken, schema::subscription_tokens::dsl::*};

#[derive(Deserialize)]
pub struct Parameters {
    subscription_token: String,
}

#[tracing::instrument(name = "Confirm a pending subscriber", skip(parameters, subscription_service))]
pub async fn confirm<S: SubscriptionService>(
    parameters: web::Query<Parameters>,
    // pool: web::Data<Pool<ConnectionManager<PgConnection>>>,
    subscription_service: web::Data<S>,
) -> HttpResponse {
    let result = subscription_service
        .confirm_subscription(&parameters.subscription_token)
        .await;

    match result {
        Ok(_) => {
            tracing::info!("Updated subscription status");
            HttpResponse::Ok().finish()
        }

        Err(_) => {
            tracing::error!("Failed to update subscription status");
            HttpResponse::InternalServerError().finish()
        }
    }
}
