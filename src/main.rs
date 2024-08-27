use std::net::TcpListener;

use diesel::r2d2::{ConnectionManager, Pool};
use diesel::PgConnection;
use env_logger::Env;
use newsletter::startup::run;
use newsletter::configuration::get_configuration;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    let config = get_configuration().expect("Failed to get configuration");
    let address = format!("127.0.0.1:{}", config.application_port);
    let listener = TcpListener::bind(&address)?;

    let manager = ConnectionManager::<PgConnection>::new(&config.database.connection_string());
    let connection_pool = Pool::builder()
                            .test_on_check_out(true)
                            .build(manager)
                            .expect("Failed to build database connection pool");

    run(listener, connection_pool)?.await
}
