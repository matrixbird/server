//mod auth;
//pub use auth::*;

use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::Row;
use std::process;

use crate::config::Config;

#[derive(Clone)]
pub struct Database {
    pub pool: PgPool,
}

#[async_trait::async_trait]
pub trait Queries {
    async fn email_exists(&self, email: &str) -> Result<bool, anyhow::Error>;
    async fn add_email(&self, user_id: &str, email: &str) -> Result<(), anyhow::Error>;
}

impl Database {
    pub async fn new(config: &Config) -> Self {

        let db_connection_str = config.db.url.clone();

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&db_connection_str)
            .await;

        match pool {
            Ok(pool) => Self { pool },
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

    }
}
