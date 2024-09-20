use std::i16;

use actix_web::{body::to_bytes, http::StatusCode, web, HttpResponse};
use anyhow::Context;
use chrono::Utc;
use diesel::{insert_into, r2d2::ConnectionManager, PgConnection};
use r2d2::Pool;
use uuid::Uuid;

use crate::models::{InsertResponse, SavedHeader, SavedResponse};

use super::IdempotencyKey;

pub async fn get_saved_response(
    pool: &Pool<ConnectionManager<PgConnection>>,
    idempotency_key_val: &IdempotencyKey,
    uid: Uuid
) -> Result<Option<HttpResponse>, anyhow::Error> {
    use diesel::prelude::*;
    use crate::schema::idempotency::dsl::*;

    let mut conn = pool.get()?;
    let val = idempotency_key_val.clone(); 

    let current_span = tracing::Span::current();
    let saved_response: Option<SavedResponse> = web::block(move || {
        current_span.in_scope(||{
            idempotency
                    .select((response_status_code, response_headers, response_body))
                    .filter(user_id.eq(uid))
                    .filter(idempotency_key.eq(val.as_ref()))
                    .first::<SavedResponse>(&mut conn)
                    .optional()
                    .context("Failed to fetch saved response from DB")
        })
    })
    .await
    .context("Failed to executed due to threadpool error")??;

    if let Some(r) = saved_response {
        if r.response_status_code.is_none(){
            return Ok(None)
        }

        let status_code = StatusCode::from_u16(
            r.response_status_code.unwrap().try_into()?
        )?;

        let mut response = HttpResponse::build(status_code);
        for header in r.response_headers.unwrap() {
            if let Some(SavedHeader { name, value }) = header {
                response.append_header((name, value));
            }
        }

        Ok(Some(response.body(r.response_body.unwrap())))
    } else {
        Ok(None)
    }
}

pub async fn save_response(
        pool: &Pool<ConnectionManager<PgConnection>>,
        idempotency_key_val: &IdempotencyKey,
        uid: Uuid,
        http_response: HttpResponse
) -> Result<HttpResponse, anyhow::Error>{
    let (response_head, body) = http_response.into_parts();
    let body = to_bytes(body).await.map_err(|e| anyhow::anyhow!("{}", e))?;
    let status_code = response_head.status().as_u16() as i16;


    let headers = {
            let mut h = Vec::with_capacity(response_head.headers().len());
            for (name, value) in response_head.headers().iter() {
                let name = name.as_str().to_owned();
                let value = value.as_bytes().to_owned();
                h.push(Some(SavedHeader{ name, value }));
            }
        h
    };

    let val = idempotency_key_val.clone();
    let mut conn = pool.get()?;
    let body_clone = body.clone();

    let current_span = tracing::Span::current();
    web::block(move ||
        current_span.in_scope(|| {
            use diesel::prelude::*;
            use crate::schema::idempotency::dsl::*;

            diesel::update(
                idempotency.filter(
                    user_id.eq(uid)
                    .and(idempotency_key.eq(val.as_ref().to_string()))
                )
            )
            .set((
                response_status_code.eq(status_code),
                response_headers.eq(headers),
                response_body.eq(body_clone.as_ref())
            ))
            .execute(&mut conn)
            .context("Failed to update saved response")
        })
    )
    .await
    .context("Failed due to threadpool error")??;


    let http_response = response_head.set_body(body).map_into_boxed_body();
    Ok(http_response)
}

pub enum NextAction{
    StartProcessing,
    ReturnSavedResponse(HttpResponse)
}

pub async fn try_processing(pool: &Pool<ConnectionManager<PgConnection>>, idempotency_key_val: &IdempotencyKey, uid: Uuid) -> Result<NextAction, anyhow::Error>{
    use diesel::prelude::*;
    use crate::schema::idempotency::dsl::*;

    let ins = InsertResponse{
        user_id: uid,
        idempotency_key: idempotency_key_val.as_ref().to_string(),
        response_status_code: None,
        response_body: None,
        response_headers: None,
        created_at: Utc::now()
    };

    let mut conn = pool.get()?;
    let current_span = tracing::Span::current();

    let rows_affected = web::block(move || {
        current_span.in_scope(||{
            diesel::insert_into(idempotency)
                .values(ins)
                .on_conflict_do_nothing()
                .execute(&mut conn)
                .context("Failed to insert user_id and idempotency")
        })
    })
    .await
    .context("Failed due to threadpool error")??;

    if rows_affected > 0{
        Ok(NextAction::StartProcessing)
    } else {
        let mut saved_response = get_saved_response(pool, idempotency_key_val, uid)
            .await?;

        while saved_response.is_none(){
            saved_response = get_saved_response(pool, idempotency_key_val, uid).await?;
        }

        Ok(NextAction::ReturnSavedResponse(saved_response.unwrap()))
    }
}
