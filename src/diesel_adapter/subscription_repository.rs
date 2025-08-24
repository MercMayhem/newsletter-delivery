use actix_web::web;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::PgConnection;
use diesel::prelude::*;
use anyhow::Context;

use crate::domain::new_subscriber::NewSubscriber;
use crate::models::SubscriptionToken;
use crate::routes::subscribe::{insert_subscriber, InsertSubscriberError};
use crate::traits::SubscriptionRepository;
use crate::schema::subscriptions;
use crate::schema::subscription_tokens;

#[derive(Clone)]
pub struct DieselSubscriptionRepository{
    pool: web::Data<Pool<ConnectionManager<PgConnection>>>
}

impl DieselSubscriptionRepository {
    pub fn new(pool: web::Data<Pool<ConnectionManager<PgConnection>>>) -> Self {
        Self { pool }
    }
}

impl SubscriptionRepository for DieselSubscriptionRepository {
    async fn insert_subscriber(&self, form: &NewSubscriber) -> Result<String, InsertSubscriberError> {
        insert_subscriber(&self.pool, form).await
    }

    async fn confirm_subscriber(&self, subscription_token: &str) -> Result<(), anyhow::Error> {
        let mut conn = self.pool.get().unwrap();
        let subscription_token = subscription_token.to_string();

        let result = web::block(move || {
            subscription_tokens::table
                .filter(subscription_tokens::subscription_token.eq(subscription_token.clone()))
                .first::<SubscriptionToken>(&mut conn)
        })
        .await
        .unwrap()
        .context("Failed to fetch subscriber_id from subscription_tokens table")?;

        let mut conn = self.pool.get().unwrap();
        web::block(move || {
            diesel::update(subscriptions::dsl::subscriptions)
                .filter(subscriptions::dsl::id.eq(result.subscriber_id))
                .set(subscriptions::dsl::status.eq("confirmed"))
                .execute(&mut conn)
        })
        .await
        .unwrap()
        .context("Failed to update subscription status")?;

        Ok(())
    }
}
