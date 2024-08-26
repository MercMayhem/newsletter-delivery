use std::net::TcpListener;

use newsletter::run;
use newsletter::configuration::get_configuration;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let config = get_configuration().expect("Failed to get configuration");
    let address = format!("127.0.0.1:{}", config.application_port);
    let listener = TcpListener::bind(&address)?;
    run(listener)?.await
}
