use std::fmt::Write;

use actix_web::HttpResponse;
use actix_web_flash_messages::IncomingFlashMessages;

pub async fn newsletter_delivery_form(flash_messages: IncomingFlashMessages) -> HttpResponse{
    let mut msg_html = String::new();
    for m in flash_messages.iter() {
        writeln!(msg_html, "<p><i>{}</i></p>", m.content()).unwrap();
    }
    let idempotency_key = uuid::Uuid::new_v4();
    HttpResponse::Ok().body(format!(r#"
        <!DOCTYPE html>
        <html lang="en">
        <head>
            <meta charset="UTF-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <title>Post a newsletter</title>
        </head>
        <body>
            <h1>Submit Your Newsletter</h1>
            {}
            <form action="/admin/newsletter" method="POST">
                <label for="title">Title:</label><br>
                <input type="text" id="title" name="title" required><br><br>

                <label for="text">Content:</label><br>
                <input type="text" id="text" name="text" required><br><br>

                <label for="html">HTML:</label><br>
                <input type="text" id="html" name="html" required><br><br>

                <input hidden type="text" name="idempotency_key" value="{}">

                <input type="submit" value="Submit">
            </form>
        </body>
        </html>
    "#, msg_html, idempotency_key))
}
