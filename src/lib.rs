pub mod config;
pub mod appservice;
pub mod db;
pub mod server;
pub mod hook;
pub mod ping;
pub mod crypto;
pub mod api;
pub mod tasks;
pub mod auth;
pub mod middleware;
pub mod templates;
pub mod email;
pub mod session;
pub mod domain;
pub mod error;
pub mod utils;

//use tokio::time::{interval, Duration};

use std::sync::Arc;
use axum::body::Body;
use hyper_util::{client::legacy::connect::HttpConnector, rt::TokioExecutor};

pub type ProxyClient = hyper_util::client::legacy::Client<HttpConnector, Body>;

#[derive(Clone)]
pub struct AppState {
    pub mode: String,
    pub config: config::Config,
    pub db: db::Database,
    pub proxy: ProxyClient,
    pub appservice: appservice::AppService,
    pub transaction_store: ping::TransactionStore,
    pub session: session::SessionStore,
    pub mail: email::MailService,
    pub email_providers: email::EmailProviders,
    pub templates: templates::EmailTemplates,
    pub keys: crypto::Keys,
}

impl AppState {
    pub async fn new(config: config::Config) -> Result<Arc<Self>, anyhow::Error> {
        let client: ProxyClient =
            hyper_util::client::legacy::Client::<(), ()>::builder(TokioExecutor::new())
                .build(HttpConnector::new());

        let appservice = appservice::AppService::new(&config).await?;

        let session = session::SessionStore::new(&config).await?;

        let transaction_store = ping::TransactionStore::new();

        let db = db::Database::new(&config).await;

        let templates = templates::EmailTemplates::new()?;

        let mail = email::MailService::new(&config, templates.clone());

        let providers = email::EmailProviders::new("data/providers.json")?;

        let keys = crypto::Keys::new()?;

        let mode = match &config.mode {
            Some(mode) => {
                if mode == "development" {
                    "development".to_string()
                } else {
                    "production".to_string()
                }
            }
            None => "production".to_string(),
        };

        println!("Running in {} mode", mode);

        let state = Arc::new(Self {
            mode,
            config,
            db,
            proxy: client,
            appservice,
            transaction_store,
            session,
            mail,
            email_providers: providers,
            templates,
            keys,
        });

        let cron_state = state.clone();

        /*
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(60 * 5)); 
            
            loop {
                interval.tick().await;
                tasks::process_failed_emails(cron_state.clone()).await;
            }
        });
        */

        Ok(state)
    }

    pub async fn mxid_from_localpart(&self, localpart: &str) -> Result<String, anyhow::Error> {
        let user_id = format!("@{}:{}", localpart, self.config.matrix.server_name);
        Ok(user_id)
    }

    pub fn development_mode(&self) -> bool {
        self.mode == "development"
    }
}


use clap::{Parser, Subcommand};

#[derive(Parser)]
pub struct Args {
    #[arg(short, long, default_value = "config.toml")]
    pub config: std::path::PathBuf,
    #[arg(short, long, default_value = "8989")]
    pub port: u16,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Send test emails
    SendEmails {
        #[arg(long)]
        dry_run: bool,
    },
    /// Run database migrations
    Migrate,
}

impl Args {
    pub fn build() -> Self {
        Args::parse()
    }
}

