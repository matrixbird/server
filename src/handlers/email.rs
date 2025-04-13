pub use crate::AppState;

use serde_json::json;
use std::sync::Arc;

use axum::{
    extract::{State, Multipart, Path},
    response::IntoResponse,
    http::StatusCode,
    Json,
};

use mail_parser::MessageParser;

use tracing::{info, error};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct IncomingEmail {
    pub sender: String,
    pub recipient: String,
    pub raw_email: String,
}

pub async fn incoming_email(
    State(state): State<Arc<AppState>>,
    Path(params): Path<(String, String)>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let (sender, recipient) = params;
    info!("Received HTTP email from {} to {}", sender, recipient);

    let mut raw_email = String::new();
    let mut field_count = 0;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        error!("Failed to get next field: {}", e);
        StatusCode::BAD_REQUEST
    })? {
        field_count += 1;
        let name = field.name().unwrap_or("(no name)");
        let file_name = field.file_name().unwrap_or("(no file name)");
        info!("Processing field #{}: name='{}', filename='{}'", field_count, name, file_name);

        if name == "email" || name == "(no name)" {
            let data = field.bytes().await.map_err(|e| {
                error!("Failed to read field bytes: {}", e);
                StatusCode::BAD_REQUEST
            })?;

            info!("Received {} bytes of email data", data.len());
            raw_email = String::from_utf8_lossy(&data).to_string();
            break;
        }
    }

    if raw_email.is_empty() {
        error!("No email content found in multipart request");
        return Err(StatusCode::BAD_REQUEST);
    }

    info!("Successfully extracted email content ({} bytes)", raw_email.len());

    let message = MessageParser::default().parse(&raw_email).unwrap();

    println!("Parsed email: {:#?}", message.body_text(0));

    let message_id = message.message_id().unwrap_or("(no message ID)");

    println!("Message ID: {}", message_id);
    println!("Message ID: {}", message_id);

    // Process the email same as the JSON endpoint
    let payload = IncomingEmail {
        sender,
        recipient,
        raw_email,
    };

    //println!("Payload is {:#?}", payload);

    info!("Forwarding to handle_incoming_email");
    process_email(state.clone(), Json(payload)).await
}

// Handle incoming emails from Postfix
async fn process_email(
    state: Arc<AppState>,
    Json(payload): Json<IncomingEmail>,
) -> Result<impl IntoResponse, StatusCode> {
    info!("Received email from {} to {}", payload.sender, payload.recipient);

    Ok(StatusCode::OK)
}


