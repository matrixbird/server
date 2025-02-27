use sqlx::postgres::{PgPool, PgPoolOptions, PgConnectOptions};
use sqlx::ConnectOptions;
use sqlx::Row;
use std::process;

use serde_json::Value;


use crate::config::Config;

#[derive(Clone)]
pub struct Database {
    pub pool: PgPool,
}

#[derive(Debug, Clone)]
#[derive(sqlx::FromRow)]
pub struct UnprocessedEmail { 
    pub message_id: String, 
    pub envelope_to: String,
    pub email_json: Value
}

impl Database {
    pub async fn new(config: &Config) -> Self {

        let pool: PgPool;
        let mut opts: PgConnectOptions = config.db.url.clone().parse().unwrap();
        opts = opts.log_statements(tracing::log::LevelFilter::Debug)
               .log_slow_statements(tracing::log::LevelFilter::Warn, std::time::Duration::from_secs(1));


        let pg_pool = PgPoolOptions::new()
            .max_connections(5)
            .min_connections(1)
            .connect_with(opts)
            .await;

        match pg_pool {
            Ok(p) => {
                tracing::info!("Successfully connected to database");
                pool = p
            }
            Err(e) => {
                tracing::error!("Database Error:");

                let mut error: &dyn std::error::Error = &e;
                tracing::error!("Error: {}", error);

                while let Some(source) = error.source() {
                    tracing::error!("Caused by: {}", source);
                    error = source;
                }
                tracing::error!("Matrixbird cannot start without a valid database connection");

                process::exit(1);
            }
        }

        Self {
            pool,
        }

    }

    pub async fn store_email_data(
        &self, 
        message_id: &str, 
        envelope_from: &str, 
        envelope_to: &str,
        email_json: Value,
    ) 
    -> Result<(), sqlx::Error> {

        sqlx::query("INSERT INTO emails (message_id, envelope_from, envelope_to, email_json) VALUES ($1, $2, $3, $4)")
            .bind(message_id)
            .bind(envelope_from)
            .bind(envelope_to)
            .bind(email_json)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_email_processed(&self, message_id: &str) -> Result<(), anyhow::Error> {

        let now = sqlx::types::time::OffsetDateTime::now_utc();

        sqlx::query("UPDATE emails SET processed = true, processed_at = $1 WHERE message_id = $2;")
            .bind(now)
            .bind(message_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn get_unprocessed_emails(&self) -> Result<Vec<UnprocessedEmail>, anyhow::Error> {

        let emails = sqlx::query_as::<_, UnprocessedEmail>("SELECT message_id, envelope_to, email_json FROM emails WHERE processed = false ORDER BY created_at ASC;")
            .fetch_all(&self.pool)
            .await?;

        Ok(emails)
    }

    pub async fn add_invite(&self, email: &str, code: &str) -> Result<(), anyhow::Error> {

        sqlx::query("INSERT INTO invites (email, code) VALUES ($1, $2);")
            .bind(email)
            .bind(code)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn get_invite_code_email(&self, code: &str) -> Result<Option<String>, anyhow::Error> {

        let row = sqlx::query("SELECT email FROM invites WHERE code = $1 and activated = false and invite_sent = true;")
            .bind(code)
            .fetch_one(&self.pool)
            .await?;

        Ok(row.try_get("email").ok())
    }

    pub async fn activate_invite_code(&self, email: &str, code: &str) -> Result<(), anyhow::Error> {

        let now = sqlx::types::time::OffsetDateTime::now_utc();

        println!("Activating invite code: {} for email: {}", code, email);

        sqlx::query("UPDATE invites SET activated = true, activated_at = $1 WHERE email = $2 and code = $3;")
            .bind(now)
            .bind(email)
            .bind(code)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

}

impl Database {

    pub async fn email_exists(&self, email: &str) -> Result<bool, anyhow::Error>{
        let row = sqlx::query("SELECT EXISTS(SELECT 1 FROM users WHERE email = $1)")
            .bind(email)
            .fetch_one(&self.pool)
            .await?;

        let exists: bool = row.get(0);
        Ok(exists)
    }

    pub async fn user_exists(&self, user_id: &str) -> Result<bool, anyhow::Error>{
        let row = sqlx::query("SELECT EXISTS(SELECT 1 FROM users WHERE user_id = $1 and active = true)")
            .bind(user_id)
            .fetch_one(&self.pool)
            .await?;

        let exists: bool = row.get(0);
        Ok(exists)
    }

    pub async fn create_user(&self, user_id: &str, local_part: &str) -> Result<(), anyhow::Error> {

        sqlx::query("INSERT INTO users (user_id, local_part) VALUES ($1, $2);")
            .bind(user_id)
            .bind(local_part)
            .execute(&self.pool)
            .await?;

        Ok(())
    }


    pub async fn add_email(&self, user_id: &str, email: &str) -> Result<(), anyhow::Error> {

        sqlx::query("UPDATE iusers SET email = $1 WHERE user_id = $2;")
            .bind(email)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_user_id_from_email(&self, email: &str) -> Result<Option<String>, anyhow::Error> {

        let row = sqlx::query("SELECT user_id FROM users WHERE email = $1;")
            .bind(email)
            .fetch_one(&self.pool)
            .await?;

        Ok(row.try_get("user_id").ok())
    }

    pub async fn get_email_from_user_id(&self, user_id: &str) -> Result<Option<String>, anyhow::Error> {

        let row = sqlx::query("SELECT email FROM users WHERE user_id = $1;")
            .bind(user_id)
            .fetch_one(&self.pool)
            .await?;

        Ok(row.try_get("address").ok())
    }

    pub async fn store_event(
        &self, 
        event_id: &str, 
        room_id: &str, 
        event_type: &str, 
        sender: &str,
        event_json: Value,
    ) 
    -> Result<(), sqlx::Error> {

        sqlx::query("INSERT INTO events (event_id, room_id, type, sender, json) VALUES ($1, $2, $3, $4, $5)")
            .bind(event_id)
            .bind(room_id)
            .bind(event_type)
            .bind(sender)
            .bind(event_json)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

}
