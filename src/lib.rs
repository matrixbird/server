pub mod config;
pub mod appservice;
pub mod db;
pub mod server;
pub mod ping;
pub mod api;
pub mod rooms;
pub mod middleware;
pub mod cache;
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
}

impl AppState {
    pub async fn new(config: config::Config) -> Result<Arc<Self>, anyhow::Error> {
        let client: ProxyClient =
            hyper_util::client::legacy::Client::<(), ()>::builder(TokioExecutor::new())
                .build(HttpConnector::new());

        let appservice = appservice::AppService::new(&config).await?;

        let cache = cache::Cache::new(&config).await?;

        let transaction_store = ping::TransactionStore::new();

        let db = db::Database::new(&config).await;

        Ok(Arc::new(Self {
            config,
            db,
            proxy: client,
            appservice,
            transaction_store,
            cache: cache.client,
        }))
    }

    pub async fn mxid_from_localpart(&self, localpart: &str) -> Result<String, anyhow::Error> {
        let user_id = format!("@{}:{}", localpart, self.config.matrix.server_name);
        Ok(user_id)
    }
}

use clap::Parser;

#[derive(Parser)]
pub struct Args {
    #[arg(short, long, default_value = "config.toml")]
    pub config: std::path::PathBuf,
    #[arg(short, long, default_value = "8989")]
    pub port: u16,
}

impl Args {
    pub fn build() -> Self {
        Args::parse()
    }
}

