pub mod config;
pub mod appservice;
pub mod db;
pub mod server;
pub mod hook;
pub mod ping;
pub mod api;
pub mod tasks;
pub mod auth;
pub mod middleware;
pub mod templates;
pub mod cache;
pub mod email;
pub mod session;
pub mod error;
pub mod utils;

use std::sync::Arc;
use axum::body::Body;
use hyper_util::{client::legacy::connect::HttpConnector, rt::TokioExecutor};

pub type ProxyClient = hyper_util::client::legacy::Client<HttpConnector, Body>;

#[derive(Clone)]
pub struct AppState {
    pub config: config::Config,
    pub db: db::Database,
    pub proxy: ProxyClient,
    pub appservice: appservice::AppService,
    pub transaction_store: ping::TransactionStore,
    pub cache: redis::Client,
    pub session: session::SessionStore,
    pub mailer: email::Mailer,
    pub email_providers: email::EmailProviders,
    pub templates: templates::EmailTemplates,
}

impl AppState {
    pub async fn new(config: config::Config) -> Result<Arc<Self>, anyhow::Error> {
        let client: ProxyClient =
            hyper_util::client::legacy::Client::<(), ()>::builder(TokioExecutor::new())
                .build(HttpConnector::new());

        let appservice = appservice::AppService::new(&config).await?;

        let cache = cache::Cache::new(&config).await?;
        let session = session::SessionStore::new(&config).await?;

        let transaction_store = ping::TransactionStore::new();

        let db = db::Database::new(&config).await;

        if let Ok(exists) = db.user_exists(
            "@lolo:localhost:8480"
        ).await{
            println!("does user exist? {:?}", exists);
        }

        let mailer = email::Mailer::new(
            &config.mailer.api_token,
            &config.mailer.account,
        );

        let templates = templates::EmailTemplates::new()?;


        let providers = email::EmailProviders::new("providers.json")?;

        Ok(Arc::new(Self {
            config,
            db,
            proxy: client,
            appservice,
            transaction_store,
            cache: cache.client,
            session,
            mailer,
            email_providers: providers,
            templates,
        }))
    }

    pub async fn mxid_from_localpart(&self, localpart: &str) -> Result<String, anyhow::Error> {
        let user_id = format!("@{}:{}", localpart, self.config.matrix.server_name);
        Ok(user_id)
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

