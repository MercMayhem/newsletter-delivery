use std::time::Duration;

use actix_web::web;
use reqwest::blocking::Client;
use secrecy::{ExposeSecret, Secret};

use crate::{domain::subscriber_email::SubscriberEmail, email_client::SendEmailRequest};

#[derive(Clone)]
pub struct BlockEmailClient{
    http_client: Client,
    base_url: String,
    sender: SubscriberEmail,
    authorization_token: Secret<String>,
}

impl BlockEmailClient{
    pub fn new(
        base_url: String,
        sender: SubscriberEmail,
        authorization_token: Secret<String>,
        timeout: u64,
    ) -> BlockEmailClient{

        let http_client = Client::builder()
            .timeout(Duration::from_secs(timeout))
            .build()
            .unwrap();

        Self {
            http_client,
            base_url,
            sender,
            authorization_token,
        }
    }


    pub fn send_email(
        &self,
        recipient: &SubscriberEmail,
        subject: &str,
        html_content: &str,
        text_content: &str,
    ) -> Result<(), reqwest::Error> {
        let url = format!("{}/email", self.base_url);
        let request_body = SendEmailRequest{
            from: &self.sender.inner(),
            to: &recipient.inner(),
            subject,
            html_body: html_content,
            text_body: text_content,
        };

        self.http_client
            .post(url)
            .json(&request_body)
            .header(
                "X-Postmark-Server-Token",
                self.authorization_token.expose_secret(),
            )
            .send()?
            .error_for_status()?;

        Ok(())
    }

}
