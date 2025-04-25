pub mod lmtp;
pub mod middleware;

use axum::{
    middleware::{self as axum_middleware},
    routing::{get, put, post},
    http::HeaderValue,
    extract::{Request, State},
    response::{IntoResponse, Redirect},
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

use crate::config::{
    Config,
    IncomingEmailMode,
};

use middleware::{
    authenticate_homeserver,
    authenticate_incoming_email
};

use crate::handlers::ping::ping;

use crate::handlers::auth::{
    login, 
    signup, 
    password_reset,
    update_password,
    verify_password_reset_code,
    verify_email, 
    verify_code,
    username_available, 
    validate_session, 
    revoke_session,
    request_invite,
    validate_invite_code
};
use crate::handlers::features::{
    features,
    authentication_features
};

use crate::domain::{
    is_matrix_email,
    validate_domain, 
    homeserver
};

use crate::email::incoming;

use crate::crypto::verify_key;

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

        layer = match &config.server.http.allow_origin {
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

        let addr = format!("{}:{}", &self.state.config.server.http.host, &self.state.config.server.http.port);

        let service_routes = Router::new()
            .route("/_matrix/app/v1/ping", post(ping))
            .route("/_matrix/app/v1/transactions/{txn_id}", put(transactions))
            .route_layer(axum_middleware::from_fn_with_state(self.state.clone(), authenticate_homeserver));

        let auth_routes = Router::new()
            .route("/auth/login", post(login))
            .route("/auth/signup", post(signup))
            .route("/auth/code/validate/{code}", get(validate_invite_code))
            .route("/auth/request/invite/{email}", get(request_invite))
            //.route("/session/validate/:device_id", get(validate_session))
            .route("/auth/session/validate", get(validate_session))
            .route("/auth/session/revoke", get(revoke_session))
            .route("/auth/username/available/{username}", get(username_available))
            .route("/auth/email/verify", post(verify_email))
            .route("/auth/password/reset", post(password_reset))
            .route("/auth/password/update", post(update_password))
            .route("/auth/password/code/verify", post(verify_password_reset_code))
            .route("/auth/code/verify", post(verify_code));

        let email_routes = Router::new()
            .route("/domain/{domain}", get(validate_domain))
            .route("/email/{email}", get(is_matrix_email))
            .route("/homeserver", get(homeserver));

        let incoming_routes = Router::new()
            .route("/email/incoming/{sender}/{recipient}", post(incoming))
            .route_layer(axum_middleware::from_fn_with_state(self.state.clone(), authenticate_incoming_email));


        let base_routes = Router::new()
            .route("/health", get(health))
            .route("/features", get(features))
            .route("/features/authentication", get(authentication_features))
            .route("/key", get(verify_key))
            .route("/version", get(version))
            .route("/identity", get(identity))
            .route("/", get(index));

        /*
        if self.state.config.email.enabled {
            base_routes = base_routes.route("/hook", post(hook));
        }
        */

        let app = Router::new()
            .merge(service_routes)
            .merge(auth_routes)
            .merge(email_routes)
            .merge(base_routes)
            .merge(incoming_routes)
            .layer(self.setup_cors(&self.state.config))
            .layer(TraceLayer::new_for_http()
                .make_span_with(|request: &Request<_>| {
                    let path = request.uri().path().to_owned();
                    let method = request.method().clone();
                    tracing::info_span!("http-request", %path, %method)
                })
                .on_request(|_request: &hyper::Request<_>, _span: &tracing::Span| {
                    tracing::event!(tracing::Level::INFO, "request received");
                })
                .on_response(|response: &hyper::Response<_>, latency: std::time::Duration, _span: &tracing::Span| {
                    let status = response.status().as_u16();
                    tracing::event!(tracing::Level::INFO, status = status, latency = ?latency, "sent response");
                })
                .on_failure(|error, _latency, _span: &tracing::Span| {
                    tracing::error!("request failed: {}", error);
                })
            )
            .with_state(self.state.clone());

        let app = NormalizePathLayer::trim_trailing_slash()
            .layer(app);


        tokio::spawn(async move {
            info!("Pinging homeserver...");
            let txn_id = ping_state.transaction_store.generate_transaction_id().await;
            let ping = ping_state.appservice.ping_homeserver(txn_id.clone()).await;
            match ping {
                Ok(_) => info!("Homeserver pinged successfully."),
                Err(e) => tracing::error!("Failed to ping homeserver: {}", e),
            }
        });


        if self.state.config.email.incoming.mode == IncomingEmailMode::LMTP {
            tracing::info!("Incoming email mode: LMTP");

            let addr = self.state.config.lmtp_addr();

            let lmtp_state = self.state.clone();

            tokio::spawn(async move {
                let _ = lmtp::start(addr, lmtp_state)
                    .await
                    .unwrap_or_else(|e| {
                        tracing::error!("Failed to start LMTP server: {}", e);
                        std::process::exit(1);
                    });
            });
        }


        if let Ok(listener) = tokio::net::TcpListener::bind(addr.clone()).await {
            tracing::info!("Listening on {}", addr);
            axum::serve(listener, ServiceExt::<Request>::into_make_service(app)).await?;
        } else {
            tracing::error!("Failed to bind to address: {}", addr);
            std::process::exit(1);
        }

        Ok(())
    }
}

pub async fn health(
    //State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ()> {

    Ok(Json(json!({
        "healthy": true,
    })))
}

pub async fn identity(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ()> {

    let user_id = format!("@{}:{}", state.config.appservice.sender_localpart, state.config.matrix.server_name);

    Ok(Json(json!({
        "appservice_user_id": user_id,
    })))
}


pub async fn index(
    State(state): State<Arc<AppState>>,
) -> Redirect {
    let domain = state.config.matrix.server_name.clone();
    let url = format!("https://{}", domain);
    Redirect::temporary(&url)
}

pub async fn version(
) -> Result<impl IntoResponse, ()> {

    let version = env!("CARGO_PKG_VERSION");
    let hash = env!("GIT_COMMIT_HASH");

    Ok(Json(json!({
        "version": version,
        "commit": hash,
    })))
}

