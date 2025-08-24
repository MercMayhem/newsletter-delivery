use crate::{domain::new_subscriber::NewSubscriber, models::SubscribeFormData, routes::subscribe::SubscribeError, traits::{EmailSender, SubscriptionRepository, SubscriptionService}};

#[derive(Clone)]
pub struct NewsletterSubscriptionService<U, V>
    where
        U: SubscriptionRepository + Send + Sync,
        V: EmailSender + Send + Sync,
{
    pub subscription_repository: U,
    pub email_sender: V,
}

impl<U, V> SubscriptionService for NewsletterSubscriptionService<U, V> 
    where
        U: SubscriptionRepository + Send + Sync,
        V: EmailSender + Send + Sync,
{
    async fn create_subscription(&self, form: SubscribeFormData) -> Result<(), SubscribeError> {
        let new_subscriber: NewSubscriber = form.try_into().map_err(SubscribeError::ValidationError)?;

        let result = self
            .subscription_repository
            .insert_subscriber(&new_subscriber)
            .await?;
        
        tracing::info!("New subscriber has been saved successfully.");

        self.email_sender
            .send_confirmation(&new_subscriber, &result)
            .await?;

        tracing::info!("Successfully sent confirmation mail");

        Ok(())
    }

    async fn confirm_subscription(&self, subscription_token: &str) -> Result<(), String> {
        let result = self
            .subscription_repository
            .confirm_subscriber(subscription_token)
            .await;

        if let Err(e) = result {
            tracing::error!("Failed to confirm subscription: {:?}", e);
            return Err("Failed to confirm subscription".into());
        }

        Ok(())
    }
}
