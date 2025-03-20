use serde::{Serialize, Deserialize};
use std::{fs, process};
use std::path::Path;


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub general: General,
    pub server: Server,
    pub db: DB,
    pub appservice: AppService,
    pub auto_join: AutoJoin,
    pub matrix: Matrix,
    pub redis: Redis,
    pub features: Features,
    pub email: Email,
    pub smtp: SMTP,
    pub cache_rules: CacheRules,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct General {
    pub mode: Option<String>,
    pub invite_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Server {
    pub port: u16,
    pub allow_origin: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Features {
    pub authentication: AuthenticationFeatures,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticationFeatures {
    pub registration_enabled: bool,
    pub require_verification: bool,
    pub require_invite_code: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoJoin {
    pub local: bool,
    pub federated: bool,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Email {
    pub incoming: IncomingEmail,
    pub outgoing: OutgoingEmail,
    pub settings: EmailSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingEmail {
    pub enabled: bool,
    pub domain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutgoingEmail {
    pub enabled: bool,
    pub domain: String,
    pub endpoint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailSettings {
    pub send_welcome_emails: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SMTP {
    pub account: String,
    pub server: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DB {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppService {
    pub id: String,
    pub sender_localpart: String,
    pub access_token: String,
    pub hs_access_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Matrix {
    pub homeserver: String,
    pub server_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Redis {
    pub session: RedisDB,
    pub cache: RedisDB,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisDB {
    pub url: String,
    #[serde(default = "default_pool_size")]
    pub pool_size: u32,
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    pub ttl: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheRules {
    pub well_known: bool,
}


fn default_pool_size() -> u32 {
    10
}

fn default_timeout_secs() -> u64 {
    5
}


impl Config {
    pub fn new(path: impl AsRef<Path>) -> Self {

        let path = path.as_ref();

        let config_content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(e) => {
                tracing::error!("Failed to read config.toml: {}", e);
                process::exit(1);
            }
        };
        
        match toml::from_str(&config_content) {
            Ok(config) => config,
            Err(e) => {
                tracing::error!("Failed to parse config.toml: {}", e);
                process::exit(1);
            }
        }
    }
}

