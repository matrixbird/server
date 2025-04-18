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

use uuid::Uuid;

use crate::utils::generate_string;

use crate::email::{
    ParsedEmail, 
    Address,
    Attachment,
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

pub async fn parse_email(
    sender: &str,
    recipient: &str,
    message: &Message<'_>,
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
        date: Utc::now().to_rfc3339(),
        content,
        attachments: None,
        in_reply_to: None,
    };

    // Parse the "to" addresses
    if let Some(to) = message.to() {
        email.to = to.iter()
        .map(|addr| {
            Address {
                name: addr.name().map(|n| n.to_string()),
                address: addr.address().unwrap_or_default().to_string(),
            }
        })
        .collect();
    };

    // Parse the "from" address, add name
    email.from.name = message.from()
        .and_then(|addrs| addrs.first())
        .filter(|addr| addr.address() == Some(sender))
        .and_then(|addr| addr.name().map(|n| n.to_string()));

    // Parse subject
    if let Some(subject) = message.subject() {
        email.subject = Some(subject.to_string());
    }

    // Parse date
    if let Some(date) = message.date() {
        let ts = date.to_timestamp();
        if let Some(date) = DateTime::from_timestamp(ts, 0) {
            email.date = date.to_rfc3339()
        }
    }

    Ok(email)
}

pub async fn process_attachments(
    state: Arc<AppState>,
    email: &mut ParsedEmail,
    message: &Message<'_>,
){
    tracing::info!("Processing attachments for email: {}", email.message_id);

    for attachment in message.attachments() {
        if !attachment.is_message() {

            let def = generate_string(16);

            let id = Uuid::new_v4();

            let file_name = attachment.attachment_name()
                .unwrap_or(&def);

            let file_path = format!("attachments/{}/{}", id, file_name);

            let uploaded = state.storage.upload(
                &file_path,
                attachment.contents()
            ).await;

            match uploaded {
                Ok(_) => {
                    println!("Uploaded attachment: {}", file_name);

                    let mime_type = match attachment.content_type() {
                        Some(mime) => {
                            let ctype = mime.ctype().to_string();
                            if let Some(subtype) = mime.subtype() {
                                format!("{}/{}", ctype, subtype)
                            } else {
                                ctype
                            }
                        }
                        None => "application/octet-stream".to_string(),
                    };

                    let item = Attachment{
                        filename: file_name.to_string(),
                        path: file_path,
                        mime_type,
                    };

                    email.attachments.get_or_insert_with(Vec::new)
                        .push(item);

                }
                Err(e) => {
                    error!("Failed to upload attachment: {}", e);
                }
            }
        }
    }
}
