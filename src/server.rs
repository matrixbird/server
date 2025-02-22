use axum::{
    middleware::{self},
    routing::{get, put, post},
    http::HeaderValue,
    extract::{Request, State},
    response::IntoResponse,
    Json,
    Router,
    ServiceExt
};
use serde_json::json;

use std::sync::Arc;
use tracing::info;

use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tower_http::normalize_path::NormalizePathLayer;
use tower::Layer;


use http::header::CONTENT_TYPE;

use anyhow;

use crate::config::Config;
use crate::middleware::authenticate_homeserver;

use crate::ping::ping;
use crate::hook::{invite_hook, hook};

use crate::auth::{
    login, 
    signup, 
    verify_email, 
    verify_code,
    username_available, 
    validate_session, 
    request_invite,
    validate_invite_code
};

use crate::api::transactions;

pub struct Server{
    state: Arc<AppState>,
}

pub use crate::AppState;

impl Server {

    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            state
        }
    }

    pub fn setup_cors(&self, config: &Config) -> CorsLayer {

        let mut layer = CorsLayer::new()
            .allow_origin(Any)
            .allow_headers(vec![CONTENT_TYPE]);

        layer = match &config.server.allow_origin {
            Some(origins) if !origins.is_empty() && 
            !origins.contains(&"".to_string()) &&
            !origins.contains(&"*".to_string()) => {
                let origins = origins.iter().filter_map(|s| s.parse::<HeaderValue>().ok()).collect::<Vec<_>>();
                layer.allow_origin(origins)
            },
            _ => layer,
        };

        layer
    }

    pub async fn run(&self) -> Result<(), anyhow::Error> {
        let ping_state = self.state.clone();

        let addr = format!("0.0.0.0:{}", &self.state.config.server.port);

        let service_routes = Router::new()
            .route("/ping", post(ping))
            .route("/transactions/:txn_id", put(transactions))
            .route_layer(middleware::from_fn_with_state(self.state.clone(), authenticate_homeserver));

        let auth_routes = Router::new()
            .route("/login", post(login))
            .route("/signup", post(signup))
            .route("/code/validate/:code", get(validate_invite_code))
            .route("/request/invite/:email", get(request_invite))
            .route("/session/validate/:device_id", get(validate_session))
            .route("/username/available/:username", get(username_available))
            .route("/email/verify", post(verify_email))
            .route("/code/verify", post(verify_code));


        let app = Router::new()
            .nest("/_matrix/app/v1", service_routes)
            .route("/hook/invite", post(invite_hook))
            .route("/hook", post(hook))
            .nest("/auth", auth_routes)
            .route("/", get(health))
            .layer(self.setup_cors(&self.state.config))
            .layer(TraceLayer::new_for_http())
            .with_state(self.state.clone());

        let app = NormalizePathLayer::trim_trailing_slash()
            .layer(app);


        tokio::spawn(async move {
            info!("Pinging homeserver...");
            let txn_id = ping_state.transaction_store.generate_transaction_id().await;
            let ping = ping_state.appservice.ping_homeserver(txn_id.clone()).await;
            match ping {
                Ok(_) => info!("Homeserver pinged successfully."),
                Err(e) => eprintln!("Failed to ping homeserver: {}", e),
            }
        });

        if let Ok(listener) = tokio::net::TcpListener::bind(addr.clone()).await {
            axum::serve(listener, ServiceExt::<Request>::into_make_service(app)).await?;
        } else {
            eprintln!("Failed to bind to address: {}", addr);
            std::process::exit(1);
        }

        Ok(())
    }
}

pub async fn health(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ()> {

    Ok(Json(json!({
        "healthy": true,
        "features": state.config.features,
    })))
}

