use config::{Config, ConfigError};
use serde_aux::field_attributes::deserialize_number_from_string;
use serde::Deserialize;
use secrecy::Secret;
use secrecy::ExposeSecret;

#[derive(Deserialize)]
pub struct Settings {
    pub database: DatabaseSettings,
    pub application: ApplicationSettings
}

#[derive(Deserialize)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: Secret<String>,

    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,

    pub host: String,
    pub database_name: String,
    pub require_ssl: bool
}

#[derive(Deserialize)]
pub struct ApplicationSettings {
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,
}

pub enum Environment {
    Local,
    Production,
}

impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Local => "local", Environment::Production => "production",
        } 
    }
}

impl TryFrom<String> for Environment {
    type Error = String;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "local" => Ok(Self::Local), 
            "production" => Ok(Self::Production), 
            other => Err(format!("{} is not a supported environment. Use either `local` or `production`.", other))
        } 
    }
}

pub fn get_configuration() -> Result<Settings, ConfigError>{
    let base_path = std::env::current_dir().expect("Failed to get current working directory");
    let configuration_path = base_path.join("configuration");

    let base_config_path = configuration_path.join("base");
    let curr_env: Environment = std::env::var("APP_ENVIRONMENT")
        .unwrap_or("local".to_string())
        .try_into()
        .expect("Failed to parse APP_ENVIRONMENT");

    let config = Config::builder()
        .add_source(config::File::from(base_config_path))
        .add_source(config::File::from(configuration_path.join(curr_env.as_str())))
        .add_source(config::Environment::with_prefix("app").separator("__"))
        .build()?;

    let settings = config.try_deserialize::<Settings>()?;
    Ok(settings)
}

impl DatabaseSettings{
    pub fn connection_string(&self) -> Secret<String>{
        let db = self.connection_string_without_db();
        let mut db_mut = db.expose_secret().clone();

        db_mut.push_str(&format!("/{}", self.database_name));
        if self.require_ssl{
            db_mut.push_str("?sslmode=require".as_ref())
        }

        Secret::new(db_mut)
    }
    
    pub fn connection_string_without_db(&self) -> Secret<String> { 
        Secret::new(format!(
            "postgres://{}:{}@{}:{}",
            self.username, self.password.expose_secret(), self.host, self.port
            )
        )
    }
}
