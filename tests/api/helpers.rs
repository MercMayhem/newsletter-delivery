use std::net::TcpListener;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::{Connection, PgConnection, RunQueryDsl};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use newsletter::telemetry::{get_subscriber, init_subscriber};
use once_cell::sync::Lazy;
use secrecy::ExposeSecret;
use uuid::Uuid;
use newsletter::configuration::{get_configuration, DatabaseSettings};
use newsletter::email_client::EmailClient;
use fake::{Faker, Fake};

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");

static TRACING: Lazy<()> = Lazy::new(|| {
    let default_filter_level = "info".to_string();
    let subscriber_name = "test".to_string();

    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::stdout);
        init_subscriber(subscriber);
    } else {
        let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::sink);
        init_subscriber(subscriber);
    };
});

pub struct TestApp {
    pub address: String,
    pub db_pool: Pool<ConnectionManager<PgConnection>>,
}

pub fn run_db_migrations(conn: &mut impl MigrationHarness<diesel::pg::Pg>) {
    conn.run_pending_migrations(MIGRATIONS).expect("Could not run migrations");
}

fn configure_database(config: &DatabaseSettings) -> Pool<ConnectionManager<PgConnection>>{
    let mut connection = PgConnection::establish(&*config.connection_string_without_db().expose_secret())
        .expect("Failed to connect to postgres database (without DB URI used)");

    let query = format!(r#"CREATE DATABASE "{}";"#, config.database_name);
    diesel::sql_query(query).execute(&mut connection).expect("Failed to create test database");

    
    let manager = ConnectionManager::<PgConnection>::new(&*config.connection_string().expose_secret());

    let pool = Pool::builder()
        .test_on_check_out(true)
        .build(manager)
        .expect("Failed to build database connection pool");

    let mut conn = pool.get().unwrap();
    run_db_migrations(&mut conn); 

    pool
}

pub fn spawn_app() -> TestApp {
    Lazy::force(&TRACING);
    let listener = TcpListener::bind("127.0.0.1:0")
        .expect("Failed to bind to random port");
    let port = listener.local_addr().unwrap().port();
    let address = format!("http://127.0.0.1:{}", port);

    let mut config = get_configuration().expect("Failed to get configuration");
    config.database.database_name = Uuid::new_v4().to_string();

    let db_pool = configure_database(&config.database);

    let sender_email = config.email_client.sender().expect("Failed to get valid sender email");
    let email_client = EmailClient::new(config.email_client.base_url, sender_email, secrecy::Secret::new(Faker.fake()), config.email_client.timeout);

    let server = newsletter::startup::run(listener, db_pool.clone(), email_client).expect("Failed to bind address");
    let _ = tokio::spawn(server);

    TestApp{ address, db_pool }
}
