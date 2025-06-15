use serde::{Serialize, Deserialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Default)]
pub struct ConfigBuilder {
    general: Option<General>,
    encryption: Option<Encryption>,
    server: Option<Server>,
    db: Option<DB>,
    appservice: Option<AppService>,
    auto_join: Option<AutoJoin>,
    matrix: Option<Matrix>,
    admin: Option<Admin>,
    redis: Option<Redis>,
    features: Option<Features>,
    email: Option<Email>,
    smtp: Option<SMTP>,
    cache_rules: Option<CacheRules>,
    storage: Option<Storage>,
}

impl ConfigBuilder {
    pub fn new() -> Self {
        ConfigBuilder::default()
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();

        let config_content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(e) => return Err(format!("Failed to read config file: {}", e)),
        };

        let config: Config = match toml::from_str(&config_content) {
            Ok(config) => config,
            Err(e) => return Err(format!("Failed to parse config file: {}", e)),
        };

        Ok(Self {
            general: Some(config.general),
            encryption: Some(config.encryption),
            server: Some(config.server),
            db: Some(config.db),
            appservice: Some(config.appservice),
            auto_join: Some(config.auto_join),
            matrix: Some(config.matrix),
            admin: Some(config.admin),
            redis: Some(config.redis),
            features: Some(config.features),
            email: Some(config.email),
            smtp: Some(config.smtp),
            cache_rules: Some(config.cache_rules),
            storage: Some(config.storage),
        })
    }

    pub fn with_server(mut self, server: Server) -> Self {
        self.server = Some(server);
        self
    }

    pub fn with_port(mut self, port: u16) -> Self {
        let server = self.server.get_or_insert(Server::default());
        server.http.port = port;
        self
    }

    pub fn with_mode(mut self, mode: String) -> Self {
        let general = self.general.get_or_insert(General::default());
        general.mode = Some(mode);
        self
    }


    pub fn build(self) -> Result<Config, anyhow::Error> {

        if self.email.as_ref().unwrap().incoming.enabled &&
            self.email.as_ref().unwrap().incoming.mode == IncomingEmailMode::LMTP && 
            self.server.is_none() {
                    return Err(anyhow::anyhow!("LMTP server configuration is required when using LMTP mode for incoming email"));
        }

        Ok(Config {
            general: self.general.unwrap_or_default(),
            encryption: self.encryption.expect("Encryption configuration is required"),
            server: self.server.unwrap_or_default(),
            db: self.db.expect("Database configuration is required"),
            appservice: self.appservice.expect("AppService configuration is required"),
            auto_join: self.auto_join.unwrap_or_default(),
            matrix: self.matrix.expect("Matrix configuration is required"),
            admin: self.admin.expect("Admin configuration is required"),
            redis: self.redis.expect("Redis configuration is required"),
            features: self.features.unwrap_or_default(),
            email: self.email.unwrap_or_default(),
            smtp: self.smtp.expect("SMTP configuration is required"),
            cache_rules: self.cache_rules.unwrap_or_default(),
            storage: self.storage.expect("Storage configuration is required"),
        })
    }
}

impl Config {
    pub fn lmtp_addr(&self) -> String {
        format!("{}:{}", self.server.lmtp.clone().unwrap_or_default().host, self.server.lmtp.clone().unwrap_or_default().port)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub general: General,
    pub encryption: Encryption,
    pub server: Server,
    pub db: DB,
    pub appservice: AppService,
    pub auto_join: AutoJoin,
    pub matrix: Matrix,
    pub admin: Admin,
    pub redis: Redis,
    pub features: Features,
    pub email: Email,
    pub smtp: SMTP,
    pub cache_rules: CacheRules,
    pub storage: Storage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct General {
    pub mode: Option<String>,
    pub invite_code: Option<String>,
}

impl Default for General {
    fn default() -> Self {
        General {
            mode: Some("production".to_string()),
            invite_code: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Encryption {
    pub secret: String,
    pub salt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Server {
    pub http: HTTP,
    pub lmtp: Option<LMTP>,
}

impl Default for Server {
    fn default() -> Self {
        Server {
            http: HTTP::default(),
            lmtp: Some(LMTP::default()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HTTP {
    pub host: String,
    pub port: u16,
    pub allow_origin: Option<Vec<String>>,
}

impl Default for HTTP {
    fn default() -> Self {
        HTTP {
            host: "0.0.0.0".to_string(),
            port: 8989,
            allow_origin: Some(vec!["".to_string()]),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LMTP {
    pub host: String,
    pub port: u16,
}

impl Default for LMTP {
    fn default() -> Self {
        LMTP {
            host: "0.0.0.0".to_string(),
            port: 2525,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Features {
    pub authentication: AuthenticationFeatures,
}
impl Default for Features {
    fn default() -> Self {
        Features {
            authentication: AuthenticationFeatures {
                registration_enabled: true,
                require_verification: true,
                require_invite_code: false,
            },
        }
    }
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

impl Default for AutoJoin {
    fn default() -> Self {
        AutoJoin {
            local: true,
            federated: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Email {
    pub incoming: IncomingEmail,
    pub outgoing: OutgoingEmail,
    pub settings: EmailSettings,
    pub domains: Option<EmailDomains>,
}

impl Default for Email {
    fn default() -> Self {
        Email {
            incoming: IncomingEmail {
                enabled: false,
                mode: IncomingEmailMode::default(),
                domain: "".to_string(),
                token: "".to_string(),
            },
            outgoing: OutgoingEmail {
                enabled: false,
                domain: "".to_string(),
                endpoint: "".to_string(),
            },
            settings: EmailSettings {
                send_welcome_emails: true,
            },
            domains: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingEmail {
    pub enabled: bool,
    #[serde(default)]
    pub mode: IncomingEmailMode,
    pub domain: String,
    pub token: String,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum IncomingEmailMode {
    #[default]
    Pipe,
    LMTP,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutgoingEmail {
    pub enabled: bool,
    pub domain: String,
    pub endpoint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailDomains {
    pub allow: Option<Vec<String>>,
    pub reject: Option<Vec<String>>,
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
pub struct Admin {
    pub user: String,
    pub password: String,
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

impl Default for CacheRules {
    fn default() -> Self {
        CacheRules {
            well_known: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Storage {
    pub access_key_id: String,
    pub access_key_secret: String,
    pub endpoint: String,
    pub bucket: String,
}

fn default_pool_size() -> u32 {
    10
}

fn default_timeout_secs() -> u64 {
    5
}

