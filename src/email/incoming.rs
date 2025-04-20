pub use crate::AppState;

use std::sync::Arc;

use axum::{
    extract::{State, Multipart, Path},
    response::IntoResponse,
    http::StatusCode,
};

//use crate::email::ParsedEmail;

use tracing::{info, error};

use crate::utils::get_localpart;

use crate::email::{
    raw_email,
    parse_message,
    parse_email,
    process_attachments,
};

pub async fn incoming(
    State(state): State<Arc<AppState>>,
    Path(params): Path<(String, String)>,
    multipart: Multipart,
) -> Result<impl IntoResponse, StatusCode> {

    let (sender, recipient) = params;
    info!("Received email from {} to {}", sender, recipient);

    // Silently ignore the email if domain is not allowed
    if !state.email.domain_allowed(&sender) {
        error!("Sender domain is not allowed: {}", sender);
        return Err(StatusCode::OK);
    }


    if !state.config.email.incoming.enabled {
        tracing::info!("Email integration is disabled. Rejecting email.");
        return Err(StatusCode::FORBIDDEN);
    }

    // Early return for postmaster or invalid localpart
    let (user, tag) = match get_localpart(recipient.clone()) {
        Some(parts) => parts,
        //None => return Err(StatusCode::FORBIDDEN),
        None => return Err(StatusCode::OK),
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
        //return Err(StatusCode::FORBIDDEN);
        return Err(StatusCode::OK);
    }

    // Get raw email from multipart
    let raw_email = match raw_email(multipart).await {
        Ok(email) => email,
        Err(_) => {
            error!("Failed to get raw email");
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    let message = match parse_message(&raw_email).await {
        Ok(message) => message,
        Err(_) => {
            error!("Failed to parse email content");
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    // Build ParsedEmail
    let mut email = match parse_email(
        &sender,
        &recipient,
        &message,
    ).await {
        Ok(email) => email,
        Err(_) => {
            error!("Failed to parse email content");
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    println!("Parsed email: {:#?}", email);

    // Let's upload the email to object storage
    let state_clone = state.clone();
    let raw = raw_email.clone();
    let key = format!("emails/{}/{}/{}", recipient, email.date, email.message_id);
    tokio::spawn(async move {
        let _ = state_clone.storage.upload(
            &key,
            raw.as_bytes(),
        ).await.map_err(|e| {
            tracing::error!("Failed to upload email: {}", e);
        });
    });

    if message.attachment_count() > 0 {
        process_attachments(state.clone(), &mut email, &message).await;
        println!("Attachments: {:#?}", email.attachments);
    };
        

    let mxid = format!("@{}:{}", user, state.config.matrix.server_name);
    tracing::info!("User exists: {}", mxid);
    tracing::info!("Processing email for MXID: {}", mxid);

    Ok(StatusCode::OK)
}

