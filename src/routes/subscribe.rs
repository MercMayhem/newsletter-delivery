use actix_web::{web, HttpResponse};
use chrono::Utc;
use diesel::{r2d2::{ConnectionManager, Pool}, PgConnection, RunQueryDsl};
use uuid::Uuid;

use crate::models::{Subscription, SubscriptionAdd};
use crate::schema::subscriptions::dsl::*;

pub async fn subscribe(form: web::Form<Subscription>, pool: web::Data<Pool<ConnectionManager<PgConnection>>>) -> HttpResponse {
    let insert = SubscriptionAdd{
        id: Uuid::new_v4(),
        email: form.email.clone(),
        name: form.name.clone(),
        subscribed_at: Utc::now()
    };

    let mut conn = pool.get().unwrap();
    let result = web::block(move || {
        diesel::insert_into(subscriptions)
        .values(insert)
        .execute(&mut conn)
    }).await;
    
    if let Ok(res) = result {
        match res{
            Ok(_) => HttpResponse::Ok().finish(),
            Err(e) => {
                println!("Failed to execute query: {}", e);
                HttpResponse::InternalServerError().finish()
            }
        }
    } else {
        println!("Failed to execute query");
        HttpResponse::InternalServerError().finish()
    }
}
