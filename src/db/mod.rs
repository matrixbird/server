//mod auth;
//pub use auth::*;
use chrono::Utc;


use sqlx::postgres::{PgPool, PgPoolOptions, PgConnectOptions};
use sqlx::ConnectOptions;
use sqlx::Row;
use std::process;

use crate::config::Config;

#[derive(Clone)]
pub struct Database {
    pub synapse: PgPool,
    pub matrixbird: PgPool,
}

#[async_trait::async_trait]
pub trait Queries {
    async fn email_exists(&self, email: &str) -> Result<bool, anyhow::Error>;
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
        opts = opts.log_statements(log::LevelFilter::Trace);

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
        opts = opts.log_statements(log::LevelFilter::Trace);


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
    async fn email_exists(&self, email: &str) -> Result<bool, anyhow::Error>{
        let row = sqlx::query("SELECT EXISTS(SELECT 1 FROM user_threepids WHERE address = $1 and medium='email')")
            .bind(email)
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

        sqlx::query!(
            r#"
            INSERT INTO invites (email, code)
            VALUES ($1, $2)
            ON CONFLICT (email) 
            DO UPDATE SET 
                code = EXCLUDED.code,
                created_at = CURRENT_TIMESTAMP
            WHERE invites.activated = false
            "#,
            email,
            code
        )
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

        sqlx::query!(
            r#"
            UPDATE invites
            SET activated = true, activated_at = $1
            WHERE email = $2 and code = $3
            "#,
            now,
            email,
            code
        )
        .execute(self)
        .await?;

        Ok(())
    }

}
