use mail_parser::MessageParser;
use chrono::{DateTime, Utc};

use axum::extract::Multipart;

use tracing::{info, error};

use crate::email::{ParsedEmail, Content};

pub async fn parse_email(
    sender: &str,
    recipient: &str,
    mut multipart: Multipart,
) -> Result<ParsedEmail, anyhow::Error> {

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

    info!("Successfully extracted email content ({} bytes)", raw_email.len());

    let message = match MessageParser::default()
        .parse(&raw_email) {
        Some(message) => message,
        None => {
            error!("Failed to parse email content");
            return Err(anyhow::anyhow!("Failed to parse email content"));
        }
    };

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
        subject: None,
        date: Utc::now(),
        content,
    };

    if let Some(subject) = message.subject() {
        email.subject = Some(subject.to_string());
    }

    if let Some(date) = message.date() {
        let ts = date.to_timestamp();
        if let Some(date) = DateTime::from_timestamp(ts, 0) {
            email.date = date;
        }
    }

    Ok(email)
}
