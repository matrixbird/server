use thiserror::Error;
use axum::{
    body::Body,
    extract::State, 
    http::{
        Request, 
        StatusCode, 
        header::AUTHORIZATION
    },
    response::{IntoResponse, Response},
    middleware::Next,
    Json,
};

use ruma::api::client::account::whoami;


use serde_json::{
    json, 
    Value
};

use std::sync::Arc;
use crate::appservice::HttpClient;

use crate::AppState;

#[derive(Error, Debug)]
pub enum MiddlewareError {
    #[error("M_FORBIDDEN")]
    Unauthorized,
    #[error("Authentication error: {0}")]
    AuthenticationError(String),
    #[error("Homeserver unreachable: {0}")]
    HomeserverError(String),
}

impl IntoResponse for MiddlewareError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            MiddlewareError::Unauthorized => (StatusCode::FORBIDDEN, self.to_string()),
            MiddlewareError::AuthenticationError(_) => (StatusCode::FORBIDDEN, self.to_string()),
            MiddlewareError::HomeserverError(_) => (StatusCode::BAD_GATEWAY, self.to_string()),

        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}


pub async fn authenticate_user(
    State(state): State<Arc<AppState>>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, MiddlewareError> {

    let auth_header = req
        .headers()
        .get(AUTHORIZATION)
        .ok_or(MiddlewareError::Unauthorized)?
        .to_str()
        .map_err(|_| MiddlewareError::Unauthorized)?;


    let token = extract_token(auth_header)
        .ok_or(MiddlewareError::Unauthorized)?;

    let client = ruma::Client::builder()
        .homeserver_url(state.config.matrix.homeserver.clone())
        .access_token(Some(token.to_string()))
        .build::<HttpClient>().await
        .map_err(|e| MiddlewareError::AuthenticationError(e.to_string()))?;


    let whoami = client
        .send_request(whoami::v3::Request::new())
        .await
        .map_err(|e| MiddlewareError::HomeserverError(e.to_string()))?;


    let data = Data {
        user_id: whoami.user_id.to_string(),
    };

    req.extensions_mut().insert(data);

    Ok(next.run(req).await)

}


pub async fn authenticate_homeserver(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
) -> Result<impl IntoResponse, (StatusCode, Json<Value>)> {

    if let Some(auth_header) = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok()) {
        if let Some(token) = extract_token(auth_header) {
            if token == state.config.appservice.hs_access_token {
                return Ok(next.run(req).await)
            }
        }
    };

    Err((
        StatusCode::UNAUTHORIZED,
        Json(json!({
            "errcode": "M_FORBIDDEN",
        }))
    ))
}

pub fn extract_token(header: &str) -> Option<&str> {
    if header.starts_with("Bearer ") {
        Some(header.trim_start_matches("Bearer ").trim())
    } else {
        None
    }
}

pub async fn authenticate_incoming_email(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {

    if let Some(auth_header) = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok()) {
        if let Some(token) = extract_token(auth_header) {
            tracing::info!("Checking incoming email token: {}", token);
            if token == state.config.email.incoming.token {
                return Ok(next.run(req).await)
            }
        }
    };

    tracing::warn!("Invalid or missing incoming email token.");
    Err(StatusCode::UNAUTHORIZED)
}


#[derive(Clone)]
pub struct Data {
    pub user_id: String,
}

