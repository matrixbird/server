use serde::{Serialize, Deserialize};
use std::{fs, process};
use std::path::Path;


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub mode: Option<String>,
    pub server: Server,
    pub db: DB,
    pub appservice: AppService,
    pub matrix: Matrix,
    pub redis: Redis,
    pub features: Features,
    pub email: Email,
    pub mailer: Mailer,
    pub authentication: Authentication,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Server {
    pub port: u16,
    pub allow_origin: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Features {
    pub registration_enabled: bool,
    pub require_verification: bool,
    pub require_invite_code: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Email {
    pub domain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mailer {
    pub api_token: String,
    pub account: String,
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
    pub rules: AppServiceRules,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppServiceRules {
    pub auto_join: bool,
    pub invite_by_local_user: bool,
    pub federation_domain_whitelist: Vec<String>,
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
pub struct Authentication {
    pub invite_code: Option<String>,
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
                eprintln!("Failed to read config.toml: {}", e);
                process::exit(1);
            }
        };
        
        match toml::from_str(&config_content) {
            Ok(config) => config,
            Err(e) => {
                eprintln!("Failed to parse config.toml: {}", e);
                process::exit(1);
            }
        }
    }
}

