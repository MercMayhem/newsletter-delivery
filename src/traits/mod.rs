use std::future::Future;

use crate::{domain::new_subscriber::NewSubscriber, models::SubscribeFormData, routes::subscribe::{InsertSubscriberError, SubscribeError}};

pub trait SubscriptionRepository {
    fn confirm_subscriber(&self, subscription_token: &str) -> impl Future<Output = Result<(), anyhow::Error>> + Send + Sync;
    fn insert_subscriber(&self, form: &NewSubscriber) -> impl Future<Output = Result<String, InsertSubscriberError>> + Send + Sync;
}

pub trait EmailSender {
    fn send_confirmation(&self, subscriber: &NewSubscriber, confirmation_token: &String) -> impl Future<Output = Result<(), reqwest::Error>> + Send + Sync;
}

pub trait SubscriptionService {
    fn create_subscription(&self, form: SubscribeFormData) -> impl Future<Output = Result<(), SubscribeError>> + Send + Sync;
    fn confirm_subscription(&self, subscription_token: &str) -> impl Future<Output = Result<(), String>> + Send + Sync;
}
