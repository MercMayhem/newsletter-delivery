
use std::net::TcpListener;

use actix_web::{dev::Server, web, App, HttpServer};
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::PgConnection;
use crate::routes::health_check::health_check;
use crate::routes::subscribe::subscribe;


pub fn run(listener: TcpListener, connection_pool: Pool<ConnectionManager<PgConnection>>) -> Result<Server, std::io::Error>{

    let connection_pool = web::Data::new(connection_pool);

    let server = HttpServer::new(move || { 
        App::new()
            .route("/health_check", web::get().to(health_check))
            .route("/subscriptions", web::post().to(subscribe))
            .app_data(connection_pool.clone())
    })
    .listen(listener)?
    .run();

    Ok(server)
}
