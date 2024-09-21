use config::{Config, ConfigError};
use secrecy::ExposeSecret;
use secrecy::Secret;
use serde::Deserialize;
use serde_aux::field_attributes::deserialize_number_from_string;

use crate::block_email_client::BlockEmailClient;
use crate::domain::subscriber_email::SubscriberEmail;

#[derive(Deserialize, Clone)]
pub struct Settings {
    pub database: DatabaseSettings,
    pub application: ApplicationSettings,
    pub email_client: EmailClientSettings,
    pub redis_uri: Secret<String>
}

#[derive(Deserialize, Clone)]
pub struct EmailClientSettings {
    pub base_url: String,
    pub sender_email: String,
    pub authorization_token: Secret<String>,
    pub timeout: u64,
}

impl EmailClientSettings{
    pub fn blocking_client(self) -> BlockEmailClient{
        let sender_email = self.sender().expect("Invalid sender email address.");
        let timeout = self.timeout;

        BlockEmailClient::new(
            self.base_url,
            sender_email,
            self.authorization_token,
            timeout
        )
    }
}

impl EmailClientSettings {
    pub fn sender(&self) -> Result<SubscriberEmail, String> {
        SubscriberEmail::parse(self.sender_email.clone())
    }
}

#[derive(Deserialize, Clone)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: Secret<String>,

    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,

    pub host: String,
    pub database_name: String,
    pub require_ssl: bool,
}

#[derive(Deserialize, Clone)]
pub struct ApplicationSettings {
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,
    pub base_url: String,
    pub hmac_secret: Secret<String>
}

pub enum Environment {
    Local,
    Production,
}

impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Local => "local",
            Environment::Production => "production",
        }
    }
}

impl TryFrom<String> for Environment {
    type Error = String;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "production" => Ok(Self::Production),
            other => Err(format!(
                "{} is not a supported environment. Use either `local` or `production`.",
                other
            )),
        }
    }
}

pub fn get_configuration() -> Result<Settings, ConfigError> {
    let base_path = std::env::current_dir().expect("Failed to get current working directory");
    let configuration_path = base_path.join("configuration");

    let base_config_path = configuration_path.join("base");
    let curr_env: Environment = std::env::var("APP_ENVIRONMENT")
        .unwrap_or("local".to_string())
        .try_into()
        .expect("Failed to parse APP_ENVIRONMENT");

    let config = Config::builder()
        .add_source(config::File::from(base_config_path))
        .add_source(config::File::from(
            configuration_path.join(curr_env.as_str()),
        ))
        .add_source(config::Environment::with_prefix("app").separator("__"))
        .build()?;

    let settings = config.try_deserialize::<Settings>()?;
    Ok(settings)
}

impl DatabaseSettings {
    pub fn connection_string(&self) -> Secret<String> {
        let db = self.connection_string_without_db();
        let mut db_mut = db.expose_secret().clone();

        db_mut.push_str(&format!("/{}", self.database_name));
        if self.require_ssl {
            db_mut.push_str("?sslmode=require".as_ref())
        }

        Secret::new(db_mut)
    }

    pub fn connection_string_without_db(&self) -> Secret<String> {
        Secret::new(format!(
            "postgres://{}:{}@{}:{}",
            self.username,
            self.password.expose_secret(),
            self.host,
            self.port
        ))
    }
}
