use std::net::TcpListener;
use std::time::Duration;

use crate::configuration::{DatabaseSettings, Settings};
use crate::email_client::EmailClient;
use crate::routes::health_check::health_check;
use crate::routes::logout::log_out;
use crate::routes::{admin_dashboard, change_password, change_password_form, home, login, login_form};
use crate::routes::newsletter_delivery::newsletter_delivery;
use crate::routes::subscribe::subscribe;
use crate::routes::subscriptions_confirm::confirm;
use actix_session::storage::RedisSessionStore;
use actix_session::SessionMiddleware;
use actix_web::cookie::Key;
use actix_web::{dev::Server, web, App, HttpServer};
use actix_web_flash_messages::storage::CookieMessageStore;
use actix_web_flash_messages::{FlashMessagesFramework, Level};
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::PgConnection;
use secrecy::{ExposeSecret, Secret};
use tracing_actix_web::TracingLogger;

pub struct Application {
    port: u16,
    server: Server,
}

pub struct ApplicationBaseUrl(pub String);

impl Application {
    pub async fn build(config: Settings) -> Result<Application, anyhow::Error> {
        let connection_pool = get_connection_pool(&config.database);

        let sender_email = config
            .email_client
            .sender()
            .expect("Failed to get valid sender email");
        let email_client = EmailClient::new(
            config.email_client.base_url.clone(),
            sender_email,
            config.email_client.authorization_token.clone(),
            config.email_client.timeout,
        );

        let address = format!("{}:{}", config.application.host, config.application.port);
        let listener = TcpListener::bind(&address)?;
        let port = listener.local_addr().unwrap().port();

        let server = run(
            listener,
            connection_pool,
            email_client,
            config.application.base_url,
            config.application.hmac_secret,
            config.redis_uri
        ).await?;
        Ok(Self { port, server })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
        self.server.await
    }
}

pub fn get_connection_pool(config: &DatabaseSettings) -> Pool<ConnectionManager<PgConnection>> {
    let manager =
        ConnectionManager::<PgConnection>::new(&*config.connection_string().expose_secret());
    Pool::builder()
        .test_on_check_out(true)
        .connection_timeout(Duration::from_secs(5))
        .build_unchecked(manager)
}

async fn run(
    listener: TcpListener,
    connection_pool: Pool<ConnectionManager<PgConnection>>,
    email_client: EmailClient,
    base_url: String,
    secret_key: Secret<String>,
    redis_uri: Secret<String>
) -> Result<Server, anyhow::Error> {
    let connection_pool = web::Data::new(connection_pool);
    let email_client = web::Data::new(email_client);
    let base_url = web::Data::new(ApplicationBaseUrl(base_url));

    let redis_store = RedisSessionStore::new(redis_uri.expose_secret()).await?;

    let key = Key::from(secret_key.expose_secret().as_bytes());
    let message_store = CookieMessageStore::builder(
        key.clone()
    ).build();
    let message_framework = FlashMessagesFramework::builder(message_store)
                                .minimum_level(Level::Debug)
                                .build();

    let server = HttpServer::new(move || {
        App::new()
            .wrap(message_framework.clone())
            .wrap(SessionMiddleware::new(redis_store.clone(), key.clone()))
            .wrap(TracingLogger::default())
            .route("/health_check", web::get().to(health_check))
            .route("/subscriptions", web::post().to(subscribe))
            .route("/subscriptions/confirm", web::get().to(confirm))
            .route("/newsletters", web::post().to(newsletter_delivery))
            .route("/", web::get().to(home))
            .route("/login", web::get().to(login_form))
            .route("/login", web::post().to(login))
            .route("/admin/dashboard", web::get().to(admin_dashboard))
            .route("/admin/password", web::get().to(change_password_form))
            .route("/admin/password", web::post().to(change_password))
            .route("/logout", web::post().to(log_out))
            .app_data(connection_pool.clone())
            .app_data(email_client.clone())
            .app_data(base_url.clone())
    })
    .listen(listener)?
    .run();

    Ok(server)
}
