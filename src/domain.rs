use axum::{
    extract::{State, Path},
    response::IntoResponse,
    Json,
};

use crate::utils::localhost_domain;

use std::sync::Arc;

use serde_json::{Value, json};

use crate::AppState;

use crate::utils::{
    get_email_domain,
    email_to_matrix_id
};

pub async fn is_matrix_email(
    State(state): State<Arc<AppState>>,
    Path(email): Path<String>,
) -> Json<Value> {


    let domain = match get_email_domain(&email) {
        Ok(domain) => {
            if state.development_mode() {
                localhost_domain(domain.to_string())
            } else {
                domain.to_string()
            }
        },
        Err(_) => {
            return Json(json!({
                "valid": false,
                "error": "Invalid email address."
            }))
        }
    };

    let valid = match query_server(state.clone(), &domain).await {
        Ok(valid) => valid,
        Err(err) => {
            println!("Error: {}", err);
            return Json(json!({
                "valid": false,
                "error": format!("{}", err)
            }))
        }
    };

    // gnarly, fix later
    if valid {
        if let Some(email) = email_to_matrix_id(&email) {
            println!("Domain is valid for email: {:#?}", email);

            let profile = state.appservice.get_profile(email).await;

            if let Some(profile) = profile {
                println!("Profile: {:#?}", profile);


                let mut res = json!({
                    "valid": true,
                });

                match profile.displayname {
                    Some(displayname) => {
                        res["displayname"] = json!(displayname);
                    },
                    None => {}
                }

                match profile.avatar_url {
                    Some(avatar_url) => {
                        res["avatar_url"] = json!(avatar_url);
                    },
                    None => {}
                }

                return Json(res)
            }
        }
    }


    Json(json!({"valid": false}))
}

pub async fn validate_domain(
    State(state): State<Arc<AppState>>,
    Path(domain): Path<String>,
) -> Json<Value> {


    let mut domain = domain.to_string();

    if state.development_mode() {
        domain = localhost_domain(domain);
    }

    let valid = match query_server(state.clone(), &domain).await {
        Ok(valid) => valid,
        Err(err) => {
            println!("Error: {}", err);
            return Json(json!({
                "valid": false,
                "error": format!("{}", err)
            }))
        }
    };

    Json(json!({"valid": valid}))
}

async fn query_server(
    state: Arc<AppState>,
    domain: &str,
) -> Result<bool, anyhow::Error> {

    tracing::info!("querying domain: {}", domain);

    let protocol = if state.development_mode() {
        "http"
    } else {
        "https"
    };

    let url = format!("{}://{}/.well-known/matrix/client", protocol, domain);

    let response = reqwest::get(url).await
        .map_err(|_| anyhow::anyhow!("Failed to query homeserver .well-known endpoint."))?;

    let json_data = response.json::<Value>().await
        .map_err(|_| anyhow::anyhow!("Failed to parse homeserver .well-known response."))?;

    let mbs = json_data
        .get("matrixbird.server")
        .and_then(|server| server.get("url"))
        .and_then(|url| url.as_str())
        .ok_or(anyhow::anyhow!("Missing or invalid matrixbird server configuration"))?;

    let url = format!("{}/homeserver", mbs);

    let response = reqwest::get(url).await
        .map_err(|_| anyhow::anyhow!("Failed to query matrixbird appservice."))?;

    let json_data = response.json::<Value>().await
        .map_err(|_| anyhow::anyhow!("Failed to parse matrixbird appservice response."))?;

    let homeserver = json_data
        .get("homeserver")
        .and_then(|url| url.as_str())
        .ok_or(anyhow::anyhow!("Missing or invalid homeserver configuration"))?;

    let mut hs = homeserver.to_string();

    if state.development_mode() {
        hs = localhost_domain(homeserver.to_string());
    }

    if hs == domain {
        tracing::info!("Domain is valid");
        return Ok(true)
    }

    Ok(false)
}


pub async fn homeserver(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ()> {

    Ok(Json(json!({
        "homeserver": state.config.matrix.server_name
    })))
}
