mod users;
mod emails;
mod events;
mod invites;
mod access_tokens;

use sqlx::postgres::{PgPool, PgPoolOptions, PgConnectOptions};
use sqlx::ConnectOptions;
use std::process;

use crate::config::Config;


pub use users::UserQueries;
pub use emails::{EmailQueries, UnprocessedEmail};
pub use events::EventQueries;
pub use access_tokens::AccessTokenQueries;
pub use invites::InviteQueries;


#[derive(Clone)]
pub struct Database {
    pub pool: PgPool,
    pub users: UserQueries,
    pub emails: EmailQueries,
    pub events: EventQueries,
    pub access_tokens: AccessTokenQueries,
    pub invites: InviteQueries,
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
            pool: pool.clone(),
            users: UserQueries::new(pool.clone()),
            emails: EmailQueries::new(pool.clone()),
            events: EventQueries::new(pool.clone()),
            access_tokens: AccessTokenQueries::new(pool.clone()),
            invites: InviteQueries::new(pool.clone()),
        }

    }

}
