//mod auth;
//pub use auth::*;
use chrono::Utc;


use sqlx::postgres::{PgPool, PgPoolOptions, PgConnectOptions};
use sqlx::ConnectOptions;
use sqlx::Row;
use std::process;

use serde_json::Value;


use crate::config::Config;

#[derive(Clone)]
pub struct Database {
    pub synapse: PgPool,
    pub matrixbird: PgPool,
}

#[async_trait::async_trait]
pub trait Queries {
    async fn store_email_data(&self, envelope_from: &str, envelope_to: &str,email: Value) -> Result<(), anyhow::Error>;
    async fn access_token_valid(&self, user_id: &str, access_token: &str,device_id: &str) -> Result<bool, anyhow::Error>;
    async fn email_exists(&self, email: &str) -> Result<bool, anyhow::Error>;
    async fn user_exists(&self, user_id: &str) -> Result<bool, anyhow::Error>;
    async fn add_email(&self, user_id: &str, email: &str) -> Result<(), anyhow::Error>;
    async fn get_user_id_from_email(&self, email: &str) -> Result<Option<String>, anyhow::Error>;
    async fn get_email_from_user_id(&self, user_id: &str) -> Result<Option<String>, anyhow::Error>;
    async fn add_invite(&self, email: &str, code: &str) -> Result<(), anyhow::Error>;
    async fn get_invite_code_email(&self, code: &str) -> Result<Option<String>, anyhow::Error>;
    async fn activate_invite_code(&self, email: &str, code: &str) -> Result<(), anyhow::Error>;
}

impl Database {
    pub async fn new(config: &Config) -> Self {

        let synapse_db: PgPool;
        let mut opts: PgConnectOptions = config.db.synapse.clone().parse().unwrap();
        opts = opts.log_statements(log::LevelFilter::Debug);

        let pool = PgPoolOptions::new()
            .max_connections(20)
            .min_connections(2)
            .connect_with(opts)
            .await;

        match pool {
            Ok(pool) => synapse_db = pool,
            Err(e) => {
                eprintln!("Database Error:\n");
                // Print the error with full context
                let mut error: &dyn std::error::Error = &e;
                eprintln!("Error: {}", error);
                while let Some(source) = error.source() {
                    eprintln!("Caused by: {}", source);
                    error = source;
                }
                eprintln!("\nSymposium cannot start without a valid database connection.");
                process::exit(1);
            }
        }

        let matrixbird_db: PgPool;
        let mut opts: PgConnectOptions = config.db.matrixbird.clone().parse().unwrap();
        opts = opts.log_statements(log::LevelFilter::Debug);


        let pool = PgPoolOptions::new()
            .max_connections(5)
            .min_connections(1)
            .connect_with(opts)
            .await;

        match pool {
            Ok(pool) => matrixbird_db = pool,
            Err(e) => {
                eprintln!("Database Error:\n");
                // Print the error with full context
                let mut error: &dyn std::error::Error = &e;
                eprintln!("Error: {}", error);
                while let Some(source) = error.source() {
                    eprintln!("Caused by: {}", source);
                    error = source;
                }
                eprintln!("\nSymposium cannot start without a valid database connection.");
                process::exit(1);
            }
        }

        Self {
            synapse: synapse_db,
            matrixbird: matrixbird_db,
        }

    }
}

#[async_trait::async_trait]
impl Queries for PgPool {

    async fn store_email_data(
        &self, 
        envelope_from: &str, 
        envelope_to: &str,
        email: Value,
    ) 
    -> Result<(), anyhow::Error> {

        sqlx::query("INSERT INTO emails (envelope_from, envelope_to, email) VALUES ($1, $2, $3)")
            .bind(envelope_from)
            .bind(envelope_to)
            .bind(email)
            .execute(self)
            .await?;
        Ok(())
    }

    async fn access_token_valid(
        &self, 
        user_id: &str,
        access_token: &str,
        device_id: &str
    ) 
    -> Result<bool, anyhow::Error>{


        println!("Checking access token: {} for user: {} and device: {}", access_token, user_id, device_id);

        let row = sqlx::query("SELECT EXISTS(SELECT 1 FROM access_tokens WHERE user_id = $1 and token = $2 and device_id = $3)")
            .bind(user_id)
            .bind(access_token)
            .bind(device_id)
            .fetch_one(self)
            .await?;

        let exists: bool = row.get(0);

        println!("Access token exists: {}", exists);

        Ok(exists)
    }

    async fn email_exists(&self, email: &str) -> Result<bool, anyhow::Error>{
        let row = sqlx::query("SELECT EXISTS(SELECT 1 FROM user_threepids WHERE address = $1 and medium='email')")
            .bind(email)
            .fetch_one(self)
            .await?;

        let exists: bool = row.get(0);
        Ok(exists)
    }

    async fn user_exists(&self, user_id: &str) -> Result<bool, anyhow::Error>{
        let row = sqlx::query("SELECT EXISTS(SELECT 1 FROM users WHERE name = $1 and deactivated = 0 and approved = true and is_guest = 0 and suspended = false )")
            .bind(user_id)
            .fetch_one(self)
            .await?;

        let exists: bool = row.get(0);
        Ok(exists)
    }


    async fn add_email(&self, user_id: &str, email: &str) -> Result<(), anyhow::Error> {

        let now = Utc::now().timestamp();

        sqlx::query("INSERT INTO user_threepids (user_id, medium, address, validated_at, added_at) VALUES ($1, $2, $3, $4, $5)")
            .bind(user_id)
            .bind("email")
            .bind(email)
            .bind(now)
            .bind(now)
            .execute(self)
            .await?;
        Ok(())
    }

    async fn get_user_id_from_email(&self, email: &str) -> Result<Option<String>, anyhow::Error> {

        let row = sqlx::query("SELECT user_id FROM user_threepids WHERE address = $1 and medium='email';")
            .bind(email)
            .fetch_one(self)
            .await?;

        Ok(row.try_get("user_id").ok())
    }

    async fn get_email_from_user_id(&self, user_id: &str) -> Result<Option<String>, anyhow::Error> {

        let row = sqlx::query("SELECT address FROM user_threepids WHERE user_id = $1 and medium='email';")
            .bind(user_id)
            .fetch_one(self)
            .await?;

        Ok(row.try_get("address").ok())
    }

    async fn add_invite(&self, email: &str, code: &str) -> Result<(), anyhow::Error> {

        sqlx::query("INSERT INTO invites (email, code) VALUES ($1, $2);")
            .bind(email)
            .bind(code)
            .execute(self)
            .await?;

        Ok(())
    }

    async fn get_invite_code_email(&self, code: &str) -> Result<Option<String>, anyhow::Error> {

        let row = sqlx::query("SELECT email FROM invites WHERE code = $1 and activated = false and invite_sent = true;")
            .bind(code)
            .fetch_one(self)
            .await?;

        Ok(row.try_get("email").ok())
    }

    async fn activate_invite_code(&self, email: &str, code: &str) -> Result<(), anyhow::Error> {

        let now = sqlx::types::time::OffsetDateTime::now_utc();

        println!("Activating invite code: {} for email: {}", code, email);

        sqlx::query("UPDATE invites SET activated = true, activated_at = $1 WHERE email = $2 and code = $3;")
            .bind(now)
            .bind(email)
            .bind(code)
            .execute(self)
            .await?;

        Ok(())
    }

}
