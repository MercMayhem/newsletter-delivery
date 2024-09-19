// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::query_builder::QueryId, Clone, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "header_pair"))]
    pub struct HeaderPair;
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::HeaderPair;

    idempotency (user_id, idempotency_key) {
        user_id -> Uuid,
        idempotency_key -> Text,
        response_status_code -> Nullable<Int2>,
        response_headers -> Nullable<Array<Nullable<HeaderPair>>>,
        response_body -> Nullable<Bytea>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    subscription_tokens (subscription_token) {
        subscription_token -> Text,
        subscriber_id -> Uuid,
    }
}

diesel::table! {
    subscriptions (id) {
        id -> Uuid,
        email -> Text,
        name -> Text,
        subscribed_at -> Timestamptz,
        status -> Text,
    }
}

diesel::table! {
    users (user_id) {
        user_id -> Uuid,
        username -> Text,
        password -> Text,
    }
}

diesel::joinable!(idempotency -> users (user_id));
diesel::joinable!(subscription_tokens -> subscriptions (subscriber_id));

diesel::allow_tables_to_appear_in_same_query!(
    idempotency,
    subscription_tokens,
    subscriptions,
    users,
);
