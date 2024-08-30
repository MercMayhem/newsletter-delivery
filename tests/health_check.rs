use std::net::TcpListener;

use diesel::{query_dsl::methods::SelectDsl, r2d2::{ConnectionManager, Pool}, Connection, PgConnection, RunQueryDsl};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use newsletter::{configuration::{get_configuration, DatabaseSettings}, email_client::EmailClient, models::Subscription, telemetry::{get_subscriber, init_subscriber}};
use once_cell::sync::Lazy;
use secrecy::ExposeSecret;
use uuid::Uuid;
    
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

#[actix_web::test]
async fn health_check_works() {
    let app = spawn_app();
    let client = reqwest::Client::new();

    let response = client
            .get(format!("{}/health_check", &app.address))
            .send()
            .await
            .expect("Failed to execute request.");

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length())
}

#[actix_web::test]
async fn subscribe_returns_a_200_for_valid_form_data() {
    use newsletter::schema::subscriptions::dsl::*;

    let app = spawn_app();
    let client = reqwest::Client::new();
    let mut conn = app.db_pool.get().unwrap();

    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";
    let response = client
        .post(&format!("{}/subscriptions", &app.address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("Failed to execute request.");

    assert_eq!(200, response.status().as_u16());

    let saved = subscriptions.select((email, name))
                    .first::<Subscription>(&mut conn)
                    .expect("Failed to fetch saved subscriptions");

    assert_eq!(saved.email, "ursula_le_guin@gmail.com");
    assert_eq!(saved.name, "le guin");
}

#[actix_web::test]
async fn subscribe_returns_a_400_when_data_is_missing() {
    let app = spawn_app();
    let client = reqwest::Client::new();
    let test_cases = vec![
            ("name=le%20guin", "missing the email"),
            ("email=ursula_le_guin%40gmail.com", "missing the name"),
            ("", "missing both name and email")
    ];

    for (invalid_body, error_message) in test_cases {
        let response = client
            .post(&format!("{}/subscriptions", &app.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(invalid_body)
            .send()
            .await
            .expect("Failed to execute request.");

        assert_eq!( 
            400,
            response.status().as_u16(),
            
            "The API did not fail with 400 Bad Request when the payload was {}.",
            error_message
        );
    }
}

#[actix_web::test]
async fn subscribe_returns_a_200_when_fields_are_present_but_empty() {
    let app = spawn_app();
    let client = reqwest::Client::new(); 
    let test_cases = vec![
        ("name=&email=ursula_le_guin%40gmail.com", "empty name"),
        ("name=Ursula&email=", "empty email"),
        ("name=Ursula&email=definitely-not-an-email", "invalid email"),
    ];
        
    for (body, description) in test_cases {
        let response = client
            .post(&format!("{}/subscriptions", &app.address)) 
            .header("Content-Type", "application/x-www-form-urlencoded") 
            .body(body)
            .send()
            .await
            .expect("Failed to execute request.");

        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not return a 200 OK when the payload was {}.",
            description
        ); 
    }
}

pub struct TestApp {
    pub address: String,
    pub db_pool: Pool<ConnectionManager<PgConnection>>,
}

pub fn run_db_migrations(conn: &mut impl MigrationHarness<diesel::pg::Pg>) {
    conn.run_pending_migrations(MIGRATIONS).expect("Could not run migrations");
}

fn configure_database(config: &DatabaseSettings) -> Pool<ConnectionManager<PgConnection>>{
    let mut connection = PgConnection::establish(&*config.connection_string_without_db().expose_secret()).expect("Failed to connect to postgres database (without DB URI used)");
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

fn spawn_app() -> TestApp {
    Lazy::force(&TRACING);
    let listener = TcpListener::bind("127.0.0.1:0")
        .expect("Failed to bind to random port");
    let port = listener.local_addr().unwrap().port();
    let address = format!("http://127.0.0.1:{}", port);

    let mut config = get_configuration().expect("Failed to get configuration");
    config.database.database_name = Uuid::new_v4().to_string();

    let db_pool = configure_database(&config.database);

    let sender_email = config.email_client.sender().expect("Failed to get valid sender email");
    let email_client = EmailClient::new(config.email_client.base_url, sender_email);

    let server = newsletter::startup::run(listener, db_pool.clone(), email_client).expect("Failed to bind address");
    let _ = tokio::spawn(server);

    TestApp{ address, db_pool }
}
