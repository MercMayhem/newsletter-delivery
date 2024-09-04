use wiremock::{matchers::{method, path}, Mock, ResponseTemplate};
use newsletter::{models::Subscription, schema::subscriptions::dsl::*, startup::get_connection_pool};
use crate::helpers::spawn_app;
use diesel::QueryDsl;
use diesel::RunQueryDsl;

#[actix_web::test]
async fn confirmations_without_token_are_rejected_with_a_400() {
    let app = spawn_app().await;

    let response = reqwest::get(&format!("{}/subscriptions/confirm", app.address))
        .await
        .unwrap();

    assert_eq!(response.status().as_u16(), 400);
}

#[actix_web::test]
async fn the_link_returned_by_subscribe_returns_a_200_if_called() {
    let app = spawn_app().await;

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";
    app.post_subscriptions(body.into()).await;

    
    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_link = app.get_confirmation_links(email_request);

    let response = reqwest::get(confirmation_link.html)
                    .await
                    .unwrap();

    assert_eq!(response.status().as_u16(), 200); 
}

#[actix_web::test]
async fn clicking_the_confirmation_link_confirms_the_subscriber() {
    let app = spawn_app().await;
    let mut conn = app.db_pool.get().unwrap();

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";
    app.post_subscriptions(body.into()).await;

    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_link = app.get_confirmation_links(email_request);

    reqwest::get(confirmation_link.html)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    
    let saved = subscriptions.select((email, name, status))
                    .first::<Subscription>(&mut conn)
                    .expect("Failed to fetch saved subscriptions");

    assert_eq!(saved.email, "ursula_le_guin@gmail.com");
    assert_eq!(saved.name, "le guin");
    assert_eq!(saved.status, "confirmed")
}
