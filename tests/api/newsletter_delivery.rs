use std::{path, time::Duration};

use wiremock::{matchers::{any, method, path}, Mock, ResponseTemplate};

use crate::helpers::{spawn_app, ConfirmationLinks, TestApp};

#[actix_web::test]
async fn newsletter_form_on_get_endpoint_works(){
    let app = spawn_app().await;

    app.post_login(&serde_json::json!({
        "username": &app.test_user.username,
        "password": &app.test_user.password
    }))
    .await;

    let response = app.get_delivery().await;
    assert_eq!(response.status().as_u16(), 200)
}

#[actix_web::test]
async fn newsletters_are_not_delivered_to_unconfirmed_subscribers() {
    let app = spawn_app().await;
    create_unconfirmed_subscriber(&app).await;

    app.post_login(&serde_json::json!({
        "username": &app.test_user.username,
        "password": &app.test_user.password
    }))
    .await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&app.email_server)
        .await;

    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter Title",
        "text": "Newsletter text",
        "html": "Newsletter html",
        "idempotency_key": uuid::Uuid::new_v4().to_string()
    });
    let response = app.post_delivery(newsletter_request_body).await;

    assert_eq!(response.status().as_u16(), 303);
    
    let html = app.get_delivery_html().await;
    assert!(html.contains("<p><i>Successfully sent newsletter.</i></p>"))
}

#[actix_web::test]
async fn newsletters_are_delivered_to_confirmed_subscribers(){
    let app = spawn_app().await;
    create_confirmed_subscriber(&app).await;

    app.post_login(&serde_json::json!({
        "username": &app.test_user.username,
        "password": &app.test_user.password
    }))
    .await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter Title",
        "text": "Newsletter text",
        "html": "Newsletter html",
        "idempotency_key": uuid::Uuid::new_v4().to_string()
    });
    let response = app.post_delivery(newsletter_request_body).await;

    assert_eq!(response.status().as_u16(), 303);
}

#[actix_web::test]
async fn newsletter_delivery_returns_400_for_invalid_data(){
    let app = spawn_app().await;

    app.post_login(&serde_json::json!({
        "username": &app.test_user.username,
        "password": &app.test_user.password
    }))
    .await;

    let test_cases = vec![
        (
            serde_json::json!({
                "text": "Newsletter text",
                "html": "Newsletter html",
                "idempotency_key": uuid::Uuid::new_v4().to_string()
            }),
            "missing title",   
        ),

        (
            serde_json::json!({
                "title": "Newsletter Title",
                "idempotency_key": uuid::Uuid::new_v4().to_string()
            }),
            "missing content"
        )
    ];

    for (invalid_body, error_message) in test_cases {
        let response = app.post_delivery(invalid_body).await;

        assert_eq!(400, response.status().as_u16(), 
            "The API did not fail with 400 Bad Request when the payload was {}.",
            error_message)
    }
}


async fn create_unconfirmed_subscriber(app: &TestApp) -> ConfirmationLinks {
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    let _mock_guard = Mock::given(any())
                        .and(method("POST"))
                        .respond_with(ResponseTemplate::new(200))
                        .named("Create unconfirmed subscriber")
                        .expect(1)
                        .mount_as_scoped(&app.email_server)
                        .await;

    app.post_subscriptions(body.into())
        .await
        .error_for_status()
        .unwrap();

    let email_request = &app
        .email_server
        .received_requests()
        .await
        .unwrap()
        .pop()
        .unwrap();

    app.get_confirmation_links(&email_request)
}

async fn create_confirmed_subscriber(app: &TestApp) {
    let confirmation_link = create_unconfirmed_subscriber(app).await;
    reqwest::get(confirmation_link.html)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
}


#[actix_web::test]
async fn newsletter_creation_is_idempotent(){
    let app = spawn_app().await;
    create_confirmed_subscriber(&app).await;

    app.post_login(&serde_json::json!({
        "username": &app.test_user.username,
        "password": &app.test_user.password
    })).await;

    Mock::given(any())
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;


    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter Title",
        "text": "Newsletter text",
        "html": "Newsletter html",
        "idempotency_key": uuid::Uuid::new_v4().to_string()
    });

    app.post_delivery(&newsletter_request_body).await;
    app.post_delivery(&newsletter_request_body).await;
}

#[actix_web::test]
async fn concurrent_form_submission_is_handled_gracefully() {
    let app = spawn_app().await;
    create_confirmed_subscriber(&app).await;

    app.post_login(&serde_json::json!({
        "username": &app.test_user.username,
        "password": &app.test_user.password
    })).await;

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter title",
        "text": "Newsletter body as plain text",
        "html": "<p>Newsletter body as HTML</p>",
        "idempotency_key": uuid::Uuid::new_v4().to_string()
    });


    let response1 = app.post_delivery(&newsletter_request_body);
    let response2 = app.post_delivery(&newsletter_request_body);

    let (response1, response2) = tokio::join!(response1, response2);

    assert_eq!(response1.status(), response2.status());
    assert_eq!(response1.text().await.unwrap(), response2.text().await.unwrap());
}
