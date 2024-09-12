use actix_web::{http::header::{ContentType, LOCATION}, web, HttpResponse};
use anyhow::Context;
use diesel::{deserialize::Queryable, r2d2::ConnectionManager, ExpressionMethods, PgConnection, QueryDsl, RunQueryDsl};
use r2d2::Pool;
use uuid::Uuid;

use crate::session_state::TypedSession;

pub async fn admin_dashboard(pool:web::Data<Pool<ConnectionManager<PgConnection>>>,session: TypedSession) -> Result<HttpResponse, actix_web::Error>{
    let username = if let Some(user_id) = session.get_user_id().map_err(e500)? {
        get_username(user_id, &pool).await.map_err(|e| e500(e))?
    } else {
        return Ok(HttpResponse::SeeOther()
            .insert_header((LOCATION, "/login"))
            .finish());
    };

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
        </ol>
    </body>
    </html>"#,
            )))
}

fn e500<T>(e: T) -> actix_web::Error 
where
    T: std::fmt::Debug + std::fmt::Display + 'static
{
        actix_web::error::ErrorInternalServerError(e)
}

#[tracing::instrument(name = "Get username", skip(pool))]
async fn get_username(uid: Uuid, pool: &Pool<ConnectionManager<PgConnection>>) -> Result<String, anyhow::Error>{
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
