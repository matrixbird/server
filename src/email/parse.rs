pub use crate::AppState;
use std::sync::Arc;

use mail_parser::{
    Message,
    MessageParser,
    MimeHeaders,
};

use chrono::{DateTime, Utc};

use axum::extract::Multipart;

use tracing::{info, error};

use crate::email::{
    ParsedEmail, 
    Address,
    Content
};

pub async fn raw_email(
    mut multipart: Multipart,
) -> Result<String, anyhow::Error> {

    let mut raw_email = String::new();
    let mut field_count = 0;

    while let Some(field) = multipart.next_field().await? {
        field_count += 1;
        let name = field.name().unwrap_or("(no name)");
        let file_name = field.file_name().unwrap_or("(no file name)");
        info!("Processing field #{}: name='{}', filename='{}'", field_count, name, file_name);

        if name == "email" || name == "(no name)" {
            let data = field.bytes().await?;

            info!("Received {} bytes of email data", data.len());
            raw_email = String::from_utf8_lossy(&data).to_string();
            break;
        }
    }

    if raw_email.is_empty() {
        return Err(anyhow::anyhow!("No email content found in multipart request"));
    }

    Ok(raw_email)
}

pub async fn parse_message(
    raw_email: &str,
) -> Result<Message, anyhow::Error> {

    let message = match MessageParser::default()
        .parse(raw_email) {
        Some(message) => message,
        None => {
            error!("Failed to parse message");
            return Err(anyhow::anyhow!("Failed to parse message"));
        }
    };

    Ok(message)
}

pub async fn parse_email<'x>(
    sender: &str,
    recipient: &str,
    message: &Message<'x>,
) -> Result<ParsedEmail, anyhow::Error> {

    let mut content = Content {
        text: None,
        html: None,
    };

    if let Some(text) = message.body_text(0) {
        content.text = Some(text.to_string());
    }

    if let Some(html) = message.body_html(0) {
        content.html = Some(html.to_string());
    }

    let mut email = ParsedEmail {
        message_id: message.message_id().unwrap_or_default().to_string(),
        sender: sender.to_string(),
        recipient: recipient.to_string(),
        from: Address {
            name: None,
            address: sender.to_string(),
        },
        to: vec![],
        subject: None,
        date: Utc::now(),
        content,
        attachments: None,
    };

    // Parse the "to" addresses
    if let Some(to) = message.to() {
        let all = to.iter()
            .filter_map(|addr| {
                let address = addr.address().unwrap_or_default();
                Some(Address {
                    name: addr.name().map(|n| n.to_string()),
                    address: address.to_string(),
                })
            })
            .collect::<Vec<_>>();
        email.to = all;
    };

    // Parse the "from" address, add name
    email.from.name = message.from()
        .and_then(|addrs| addrs.first())
        .filter(|addr| addr.address().map_or(false, |address| address == sender))
        .and_then(|addr| addr.name().map(|n| n.to_string()));

    // Parse subject
    if let Some(subject) = message.subject() {
        email.subject = Some(subject.to_string());
    }

    // Parse date
    if let Some(date) = message.date() {
        let ts = date.to_timestamp();
        if let Some(date) = DateTime::from_timestamp(ts, 0) {
            email.date = date;
        }
    }

    Ok(email)
}

pub async fn process_attachments<'x>(
    state: Arc<AppState>,
    email: &ParsedEmail,
    message: &Message<'x>,
){
    tracing::info!("Processing attachments for email: {}", email.message_id);

    for attachment in message.attachments() {
        if !attachment.is_message() {
            let uploaded = state.storage.upload(
                &format!("attachments/{}/{}", email.message_id, attachment.attachment_name()
                    .unwrap_or("(no filename)")),
                attachment.contents()
            ).await;
            match uploaded {
                Ok(_) => {
                    println!("Uploaded attachment: {}", attachment.attachment_name().unwrap_or("(no filename)"));
                }
                Err(e) => {
                    error!("Failed to upload attachment: {}", e);
                }
            }
        }
    }
}
