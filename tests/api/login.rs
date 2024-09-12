use reqwest::Response;
use std::fmt::Write;
use crate::helpers::{assert_is_redirect_to, spawn_app};


#[actix_web::test]
async fn an_error_flash_message_is_set_on_failure() {
    let app = spawn_app().await;

    let login_body = serde_json::json!({
        "username": "random-username",
        "password": "random-password"
    });

    let response = app.post_login(&login_body).await;
    assert_is_redirect_to(&response, "/login");
    println!("{}", pretty_print_response(response).await);


    let html_page = app.get_login_html().await;
    assert!(html_page.contains("<p><i>Authentication failed</i></p>"));

    let html_page = app.get_login_html().await;
    assert!(!html_page.contains("<p><i>Authentication failed</i></p>"));
}

async fn pretty_print_response(response: Response) -> String {
    let mut output = String::new();

    // Status
    writeln!(&mut output, "Status: {} {}", response.status().as_u16(), response.status().canonical_reason().unwrap_or("Unknown")).unwrap();

    // Headers
    writeln!(&mut output, "\nHeaders:").unwrap();
    for (name, value) in response.headers() {
        writeln!(&mut output, "  {}: {}", name, value.to_str().unwrap_or("Unable to decode header value")).unwrap();
    }

    // Body
    writeln!(&mut output, "\nBody:").unwrap();
    let body = response.text().await.unwrap_or_else(|_| "Unable to read body".to_string());
    writeln!(&mut output, "{}", body).unwrap();

    output
}
