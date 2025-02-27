use axum::{
    extract::{State, Path},
    response::IntoResponse,
    Json,
};

use crate::utils::localhost_domain;

use std::sync::Arc;

use serde_json::{Value, json};

use crate::AppState;

pub async fn validate_domain(
    State(state): State<Arc<AppState>>,
    Path(domain): Path<String>,
) -> Json<Value> {

    tracing::info!("Validating domain: {}", domain);

    let protocol = if state.development_mode() {
        "http"
    } else {
        "https"
    };

    let url = format!("{}://{}/.well-known/matrix/client", protocol, domain);

    let response = match reqwest::get(url).await {
        Ok(resp) => resp,
        Err(_) => {
            return Json(json!({
                "error": "Failed to query homeserver"
            }))
        }
    };

    let json_data = match response.json::<Value>().await {
        Ok(data) => data,
        Err(_) => {
            return Json(json!({
                "error": "Failed to parse homeserver response"
            }))
        }
    };

    let server_url = json_data
        .get("matrixbird.server")
        .and_then(|server| server.get("url"))
        .and_then(|url| url.as_str());

    let mbs = match server_url {
        Some(url_str) => url_str,
        None => 
            return Json(json!({"valid": false, "error": "Missing or invalid matrixbird server configuration"}))
        
    };

    let url = format!("{}/homeserver", mbs);

    let response = match reqwest::get(url).await {
        Ok(resp) => resp,
        Err(_) => {
            return Json(json!({
                "error": "Failed to query matrixbird server"
            }))
        }
    };

    let json_data = match response.json::<Value>().await {
        Ok(data) => data,
        Err(_) => {
            return Json(json!({
                "error": "Failed to parse matrixbird server response"
            }))
        }
    };

    let homeserver = json_data
        .get("homeserver")
        .and_then(|url| url.as_str());

    if let Some(hs) = homeserver {

        let mut homeserver = hs.to_string();

        if state.development_mode() {
            homeserver = localhost_domain(&hs);
        }

        if homeserver == domain {
            tracing::info!("Domain is valid");
            return Json(json!({"valid": true}))
        }

    }

    Json(json!({"valid": false}))
}


pub async fn homeserver(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ()> {

    Ok(Json(json!({
        "homeserver": state.config.matrix.server_name
    })))
}
