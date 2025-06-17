use crate::config::Config;

use std::fs;
use std::path::{Path, PathBuf};

impl Config {
    pub fn generate(filename: String) {
        println!("Generating configuration file..." );

        let path = PathBuf::from(filename);

        // Check if the file already exists
        if path.exists() {
            eprintln!("Configuration file already exists at: {}", path.display());
            std::process::exit(1);
        }

        if let Err(e) = Config::write_template_config(&path) {
            eprintln!("Error writing config template: {}", e);
            std::process::exit(1);
        } else {
            println!("Configuration file created at: {}", path.display());
        }
    }

    pub fn write_template_config(path: impl AsRef<Path>) -> Result<(), anyhow::Error> {
        fs::write(path, CONFIG_TEMPLATE)
            .map_err(|e| anyhow::anyhow!("Failed to write config file: {}", e))?;
        Ok(())
    }
}

const CONFIG_TEMPLATE: &str = r#"# Sample configuration file
# Copy this to config.toml and modify as needed

[general]
# Application mode: "production" or "development"
mode = "production"
# Optional invite code for registration
# invite_code = "your-invite-code"

[encryption]
# SECURITY: Generate secure random values for these!
# Use: openssl rand -hex 32
secret = "CHANGE-ME-GENERATE-RANDOM-32-BYTE-HEX"
salt = "CHANGE-ME-GENERATE-RANDOM-32-BYTE-HEX"

[authentication]
# Whether to generate Matrix passwords automatically
generate_matrix_passwords = false

[server]
# HTTP server configuration
[server.http]
host = "0.0.0.0"
port = 8989
allow_origin = ["*"]

# LMTP server (for email processing)
[server.lmtp]
host = "0.0.0.0"
port = 2525

[db]
# PostgreSQL database URL
url = "postgresql://user:password@localhost/database_name"

[appservice]
# Matrix Application Service configuration
id = "your-appservice-id"
sender_localpart = "bridge"
access_token = "your-access-token"
hs_access_token = "your-homeserver-access-token"

[auto_join]
# Automatically join rooms
local = true
federated = true

[matrix]
# Matrix homeserver configuration
homeserver = "https://matrix.example.com"
server_name = "example.com"

[admin]
# Admin user credentials
user = "admin"
password = "CHANGE-THIS-PASSWORD"

[redis]
# Redis for sessions
[redis.session]
url = "redis://localhost:6379/0"
pool_size = 10
timeout_secs = 5
ttl = 3600  # 1 hour

# Redis for caching
[redis.cache]
url = "redis://localhost:6379/1"
pool_size = 10
timeout_secs = 5
ttl = 1800  # 30 minutes

[features]
# Authentication features
[features.authentication]
registration_enabled = true
require_verification = true
require_invite_code = false

[email]
# Incoming email processing
[email.incoming]
enabled = false
mode = "pipe"  # or "lmtp"
domain = "mail.example.com"
token = "incoming-email-token"

# Outgoing email processing
[email.outgoing]
enabled = false
domain = "example.com"
endpoint = "https://api.example.com/email"

# Email settings
[email.settings]
send_welcome_emails = true

# Optional: Email domain filtering
# [email.domains]
# allow = ["example.com", "trusted.com"]
# reject = ["spam.com"]

[smtp]
# SMTP configuration for outgoing emails
account = "noreply@example.com"
server = "smtp.example.com"
port = 587
username = "smtp-username"
password = "smtp-password"

[cache_rules]
# Caching rules
well_known = true

[storage]
# S3-compatible storage configuration
access_key_id = "your-access-key-id"
access_key_secret = "your-access-key-secret"
endpoint = "https://s3.amazonaws.com"
bucket = "your-bucket-name"
"#;

