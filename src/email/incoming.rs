pub use crate::AppState;

use std::sync::Arc;

use axum::{
    extract::{State, Multipart, Path},
    response::IntoResponse,
    http::StatusCode,
};

use crate::email::ParsedEmail;

use tracing::{info, error};
use serde::Deserialize;

use crate::utils::get_localpart;

use crate::email::parse_email;

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
) -> impl IntoResponse {
    let (sender, recipient) = params;
    info!("Received HTTP email from {} to {}", sender, recipient);

    let email = match parse_email(
        &sender,
        &recipient,
        multipart
    ).await {
        Ok(email) => email,
        Err(_) => {
            error!("Failed to parse email content");
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    println!("Parsed email: {:#?}", email);

    process_email(state.clone(), email).await

}

// Handle incoming emails from Postfix
async fn process_email(
    state: Arc<AppState>,
    email: ParsedEmail,
) -> Result<impl IntoResponse, StatusCode> {
    info!("Received email from {} to {}", email.sender, email.recipient);
    if state.config.email.incoming.enabled == false {
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

    let mxid = format!("@{}:{}", user, state.config.matrix.server_name);
    tracing::info!("User exists: {}", mxid);
    tracing::info!("Processing email for MXID: {}", mxid);



    Ok(StatusCode::OK)
}




