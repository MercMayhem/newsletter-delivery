use actix_web::{http::header::ContentType, web, HttpResponse};
use anyhow::Context;
use diesel::{r2d2::ConnectionManager, ExpressionMethods, PgConnection, QueryDsl, RunQueryDsl};
use r2d2::Pool;
use uuid::Uuid;

use crate::{session_state::UserId, utils::e500};

pub async fn admin_dashboard(pool:web::Data<Pool<ConnectionManager<PgConnection>>>, user_id: web::ReqData<UserId>) -> Result<HttpResponse, actix_web::Error>{
    let user_id = user_id.into_inner();
    let username = get_username(*user_id, &pool).await.map_err(|e| e500(e))?;

    Ok(HttpResponse::Ok()
            .content_type(ContentType::html())
            .body(format!(
                r#"<!DOCTYPE html>
    <html lang="en">
    <head>
        <meta http-equiv="content-type" content="text/html; charset=utf-8">
        <title>Admin dashboard</title>
    </head>
    <body>
        <p>Welcome {username}!</p>
        <p>Available actions:</p>
        <ol>
            <li><a href="/admin/password">Change password</a></li>
            <li>
              <form name="logoutForm" action="/admin/logout" method="post">
                <input type="submit" value="Logout">
              </form>
            </li>
            <li><a href="/admin/newsletter">Send a newsletter issue</a></li>
        </ol>
    </body>
    </html>"#,
            )))
}

#[tracing::instrument(name = "Get username", skip(pool))]
pub async fn get_username(uid: Uuid, pool: &Pool<ConnectionManager<PgConnection>>) -> Result<String, anyhow::Error>{
    use crate::schema::users::dsl::*;
    let mut conn = pool.get().context("Failed to get DB connection from pool")?;

    let result = web::block(move || {
        users.select(username)
            .filter(user_id.eq(uid))
            .get_result::<String>(&mut conn)
            .context("Failed to get user")
    })
    .await
    .context("Failed due to threadpool error")??;

    Ok(result)
}
