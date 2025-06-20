use axum::{
    extract::{State, Path},
    response::IntoResponse,
    Json,
};

use chrono::{DateTime, Utc};

use std::sync::Arc;

use std::time::Duration;

use serde_json::{Value, json};

use serde::{Serialize, Deserialize};

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
                domain.to_string()
        },
        Err(err) => {
            tracing::error!("Error: {}", err);
            return Json(json!({
                "valid": false,
                "error": "Invalid email address."
            }))
        }
    };

    let valid = match query_server(state.clone(), &domain).await {
        Ok(valid) => valid,
        Err(err) => {
            tracing::error!("Error: {}", err);
            return Json(json!({
                "valid": false,
                "error": "Not a valid Matrix email address."
            }))
        }
    };

    // gnarly, fix later
    if valid {
        if let Some(mxid) = email_to_matrix_id(&email) {
            println!("Domain is valid for email: {:#?}", mxid);

            let profile = state.appservice.get_profile(mxid.clone()).await;

            if let Some(profile) = profile {
                println!("Profile: {:#?}", profile);

                let mut res = json!({
                    "valid": true,
                    "mxid": mxid
                });

                if let Some(displayname) = profile.displayname {
                    res["displayname"] = json!(displayname);
                }

                if let Some(avatar_url) = profile.avatar_url {
                    res["avatar_url"] = json!(avatar_url);
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


    let domain = domain.to_string();

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

#[derive(Serialize, Deserialize, Debug)]
pub struct WellKnown {
    #[serde(rename = "matrixbird.server")]
    pub matrixbird_server: MatrixbirdServer,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MatrixbirdServer {
    pub url: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct SignedMessage {
    pub message: Message,
    pub signature: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Message {
    pub homeserver: String,
    pub server_name: String,
    pub timestamp: DateTime<Utc>,
}

async fn fetch_well_known(
    well_known_url: String,
) -> Result<WellKnown, anyhow::Error> {

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .connect_timeout(Duration::from_secs(3)) 
        .build()?;

    let response = client.get(&well_known_url)
        .send()
        .await
        .map_err(|_| anyhow::anyhow!("Failed to query matrixbird server's .well-known endpoint: {}", well_known_url))?;

    let well_known = response.json::<WellKnown>().await
        .map_err(|_| anyhow::anyhow!("Failed to parse matrixbird server's .well-known response."))?;

    Ok(well_known)
}

async fn ping_appservice(
    url: &str,
) -> Result<SignedMessage, anyhow::Error> {

    let appservice_url = format!("{}/homeserver", url);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .connect_timeout(Duration::from_secs(3)) 
        .build()?;

    let response = client.get(&appservice_url)
        .send()
        .await
        .map_err(|_| anyhow::anyhow!("Failed to query appservice URL: {}", appservice_url))?;

    let message = response.json::<SignedMessage>().await
        .map_err(|_| anyhow::anyhow!("Failed to parse homeserver .well-known response."))?;

    Ok(message)
}

#[derive(Serialize, Deserialize, Debug)]
struct AppserviceKey {
    pub homeserver: String,
    pub verify_key: String,
}

async fn get_appservice_key(
    url: &str,
) -> Result<AppserviceKey, anyhow::Error> {

    let appservice_url = format!("{}/key", url);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .connect_timeout(Duration::from_secs(3)) 
        .build()?;

    let response = client.get(&appservice_url)
        .send()
        .await
        .map_err(|_| anyhow::anyhow!("Failed to query appservice URL: {}", appservice_url))?;

    let key = response.json::<AppserviceKey>().await
        .map_err(|_| anyhow::anyhow!("Failed to parse appservice key response."))?;

    Ok(key)
}


async fn query_server(
    state: Arc<AppState>,
    domain: &str,
) -> Result<bool, anyhow::Error> {

    tracing::info!("Querying remote Matrixbird server: {}", domain);

    let well_known_url = format!("https://{}/.well-known/matrixbird/server", domain);

    let well_known: WellKnown;

    if state.config.cache_rules.well_known {
        if let Some(from_cache) = state.cache.get_well_known(&well_known_url).await? {
            tracing::info!("Found cached well-known data.");
            well_known = from_cache;
        } else {
            well_known = fetch_well_known(well_known_url.to_string().clone()).await?;
        }
    } else {
        well_known = fetch_well_known(well_known_url.to_string().clone()).await?;
    }

    let key = get_appservice_key(&well_known.matrixbird_server.url).await?;

    let appservice = ping_appservice(&well_known.matrixbird_server.url).await?;

    let message_str = serde_json::to_string(&appservice.message)?;

    let valid = state.keys.verify_signature(&key.verify_key, &message_str, &appservice.signature)?;

    // signature is invalid, homeserver isn't valid
    if !valid {
        return Ok(false)
    }

    let hs = appservice.message.server_name.to_string();

    if hs == domain {
        tracing::info!("Domain is valid");

        if state.config.cache_rules.well_known {
            tokio::spawn(async move {
                let cached = state.cache.cache_well_known(
                    &well_known_url,
                    &well_known
                ).await;

                match cached {
                    Ok(_) => {
                        tracing::info!("Cached well-known value.");
                    },
                    Err(err) => {
                        tracing::error!("Failed to cache well-known: {}", err);
                    }
                }

            });
        }

        return Ok(true)
    }

    Ok(false)
}


pub async fn homeserver(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ()> {

    let payload = json!({
        "homeserver": state.config.matrix.homeserver,
        "server_name": state.config.matrix.server_name,
        "timestamp": chrono::Utc::now()
    });

    let payload_str = payload.to_string();

    let signature = state.keys.sign_message(&payload_str);

    Ok(Json(json!({
        "message": payload,
        "signature": signature
    })))
}
