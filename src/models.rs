use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::Deserialize;
use uuid::Uuid;
use crate::schema::subscriptions;

#[derive(Queryable)]
pub struct Subscription {
    pub email: String,
    pub name: String,
    pub status: String
}

#[derive(Insertable)]
#[diesel(table_name = subscriptions)]
pub struct SubscriptionAdd{
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub subscribed_at: DateTime<Utc>,
    pub status: String
}

#[derive(Deserialize)]
pub struct SubscribeFormData{
    pub email: String,
    pub name: String,
}
