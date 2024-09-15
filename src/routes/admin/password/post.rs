use actix_web::{web, HttpResponse};
use actix_web_flash_messages::FlashMessage;
use diesel::{r2d2::ConnectionManager, PgConnection};
use r2d2::Pool;
use secrecy::{ExposeSecret, Secret};
use serde::Deserialize;

use crate::{authentication::{validate_credentials, AuthError, Credentials}, routes::admin::dashboard::get_username, session_state::UserId, utils::{e500, see_other}};

#[derive(Deserialize)]
pub struct FormData{
    current_password: Secret<String>,
    new_password: Secret<String>,
    new_password_check: Secret<String>,
}

#[tracing::instrument(
    "Change current password",
    skip(form, pool, user_id)
)]
pub async fn change_password(form: web::Form<FormData>, pool: web::Data<Pool<ConnectionManager<PgConnection>>>, user_id: web::ReqData<UserId>) -> Result<HttpResponse, actix_web::Error> {
    let user_id = user_id.into_inner();

    if form.new_password.expose_secret() != form.new_password_check.expose_secret(){
        FlashMessage::error("You entered two different new passwords - the field values must match.")
            .send();

        return Ok(see_other("/admin/password"));
    }

    let username = get_username(*user_id, &pool).await.map_err(e500)?;
    let credentials = Credentials{
        username,
        password: form.0.current_password
    };

    if let Err(e) = validate_credentials(credentials, &pool).await{
        return match e {
            AuthError::InvalidCredentials(_) => {
                FlashMessage::error("The current password is incorrect.").send();
                Ok(see_other("/admin/password"))
            },

            AuthError::UnexpectedError(_) => Err(e500(e).into())
        }
    }

    crate::authentication::change_password(*user_id, form.0.new_password, &pool)
        .await
        .map_err(e500)?;

    FlashMessage::info("Your password has been changed.").send();
    Ok(see_other("/admin/password"))
}
