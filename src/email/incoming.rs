pub use crate::AppState;

use std::sync::Arc;

use axum::{
    extract::{State, Multipart, Path},
    response::IntoResponse,
    http::StatusCode,
};

//use crate::email::ParsedEmail;

use tracing::{info, error};
use serde::Deserialize;

use crate::utils::get_localpart;

use crate::email::{
    parse_email,
    get_raw_email,
};

#[derive(Debug, Deserialize)]
pub struct IncomingEmail {
    pub sender: String,
    pub recipient: String,
    pub raw_email: String,
}

pub async fn incoming(
    State(state): State<Arc<AppState>>,
    Path(params): Path<(String, String)>,
    multipart: Multipart,
) -> Result<impl IntoResponse, StatusCode> {

    let (sender, recipient) = params;
    info!("Received HTTP email from {} to {}", sender, recipient);

    let raw_email = match get_raw_email(multipart).await {
        Ok(email) => email,
        Err(_) => {
            error!("Failed to get raw email");
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    let email = match parse_email(
        &sender,
        &recipient,
        &raw_email
    ).await {
        Ok(email) => email,
        Err(_) => {
            error!("Failed to parse email content");
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    println!("Parsed email: {:#?}", email);

    if !state.config.email.incoming.enabled {
        tracing::info!("Email integration is disabled. Rejecting email.");
        return Err(StatusCode::FORBIDDEN);
    }

    // Early return for postmaster or invalid localpart
    let (user, tag) = match get_localpart(email.recipient.clone()) {
        Some(parts) => parts,
        None => return Err(StatusCode::FORBIDDEN),
    };

    if let Some(tag) = tag {
        tracing::debug!("Email tag: {}", tag);
    }

    let exists = state.appservice.user_exists(&user).await.map_err(|e| {
        tracing::error!("Failed to check user existence: {}", e);
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    if !exists {
        tracing::error!("User does not exist. Rejecting email.");
        return Err(StatusCode::FORBIDDEN);
    }

    let state_clone = state.clone();
    let key = format!("{}/{}/{}", recipient, email.date, email.message_id);
    tokio::spawn(async move {
        let _ = state_clone.storage.upload(
            &key,
            raw_email.as_bytes(),
        ).await.map_err(|e| {
            tracing::error!("Failed to upload email: {}", e);
        });
    });
        

    let mxid = format!("@{}:{}", user, state.config.matrix.server_name);
    tracing::info!("User exists: {}", mxid);
    tracing::info!("Processing email for MXID: {}", mxid);

    Ok(StatusCode::OK)
}

