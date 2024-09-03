use newsletter::startup::build;
use newsletter::configuration::get_configuration;
use newsletter::telemetry::{get_subscriber, init_subscriber};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let subscriber = get_subscriber("Newsletter".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    let config = get_configuration().expect("Failed to get configuration");
    let server = build(&config).await?;

    server.await?;
    Ok(())
}
