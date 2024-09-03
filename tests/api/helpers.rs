use diesel::r2d2::{ConnectionManager, Pool};
use diesel::{Connection, PgConnection, RunQueryDsl};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use newsletter::startup::{get_connection_pool, Application};
use newsletter::telemetry::{get_subscriber, init_subscriber};
use once_cell::sync::Lazy;
use secrecy::ExposeSecret;
use uuid::Uuid;
use newsletter::configuration::{get_configuration, DatabaseSettings};

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

impl TestApp {
    pub async fn post_subscriptions(&self, body: String) -> reqwest::Response {
        reqwest::Client::new()
            .post(&format!("{}/subscriptions", &self.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Failed to execute request")
    }
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

pub async fn spawn_app() -> TestApp {
    Lazy::force(&TRACING);

    let configuration = {
        let mut c = get_configuration().expect("Failed to read configuration.");
        c.database.database_name = Uuid::new_v4().to_string();
        c.application.port = 0;
        c
    };

    configure_database(&configuration.database);

    let application = Application::build(configuration.clone())
                        .await
                        .expect("Failed to build application.");

    let address = format!("http://127.0.0.1:{}", application.port());
    let _ = tokio::spawn(application.run_until_stopped());

    TestApp{ address, db_pool: get_connection_pool(&configuration.database)}
}
