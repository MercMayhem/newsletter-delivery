use crate::schema::idempotency;
use crate::schema::sql_types::HeaderPair;
use crate::schema::subscription_tokens;
use crate::schema::subscriptions;
use chrono::{DateTime, Utc};
use diesel::deserialize::FromSql;
use diesel::deserialize::FromSqlRow;
use diesel::expression::AsExpression;
use diesel::pg::Pg;
use diesel::pg::PgValue;
use diesel::prelude::*;
use diesel::serialize::ToSql;
use diesel::serialize::WriteTuple;
use diesel::sql_types::Bytea;
use diesel::sql_types::Record;
use diesel::sql_types::Text;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Queryable)]
pub struct SavedResponse {
    pub response_status_code: Option<i16>,
    pub response_headers: Option<Vec<Option<SavedHeader>>>,
    pub response_body: Option<Vec<u8>>,
}

#[derive(Insertable)]
#[diesel(table_name = idempotency)]
pub struct InsertResponse{
    pub user_id: Uuid,
    pub idempotency_key: String,
    pub response_status_code: Option<i16>,
    pub response_headers: Option<Vec<Option<SavedHeader>>>,
    pub response_body: Option<Vec<u8>>,
    pub created_at: DateTime<Utc>
}

#[derive(Queryable)]
pub struct Subscription {
    pub email: String,
    pub name: String,
    pub status: String,
}

#[derive(Insertable)]
#[diesel(table_name = subscriptions)]
pub struct SubscriptionAdd {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub subscribed_at: DateTime<Utc>,
    pub status: String,
}

#[derive(Insertable)]
#[diesel(table_name = subscription_tokens)]
pub struct SubscriptionTokensAdd {
    pub subscription_token: String,
    pub subscriber_id: Uuid,
}

#[derive(Queryable)]
pub struct SubscriptionToken {
    pub subscription_token: String,
    pub subscriber_id: Uuid,
}

#[derive(Deserialize)]
pub struct SubscribeFormData {
    pub email: String,
    pub name: String,
}

#[derive(Queryable, Debug)]
pub struct VerificationInfo{
    pub user_id: Uuid,
    pub password: String
}

#[derive(FromSqlRow, AsExpression, Debug)]
#[sql_type = "HeaderPair"]
pub struct SavedHeader{
    pub name: String,
    pub value: Vec<u8>
}

impl FromSql<HeaderPair, Pg> for SavedHeader{
    fn from_sql(bytes: PgValue<'_>) -> diesel::deserialize::Result<Self> {
        let (name, value) = FromSql::<Record<(Text, Bytea)>, Pg>::from_sql(bytes)?;
        Ok(Self{ name, value })
    }
}

impl ToSql<HeaderPair, Pg> for SavedHeader {
    fn to_sql<'b>(&'b self, out: &mut diesel::serialize::Output<'b, '_, Pg>) -> diesel::serialize::Result {
        WriteTuple::<(Text, Bytea)>::write_tuple(&(self.name.clone(), self.value.clone()), out)
    }
}
