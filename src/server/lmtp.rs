use tokio::net::TcpListener;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use axum::http::StatusCode;

use crate::email::{
    parse_message,
    parse_email,
    process_attachments,
};

use crate::utils::get_localpart;

use std::sync::Arc;

use crate::AppState;

use crate::tasks;

pub async fn start(
    bind: String, 
    state: Arc<AppState>
) -> Result<(), anyhow::Error> {
    let listener = TcpListener::bind(&bind).await?;
    println!("LMTP server listening on {}", bind);

    loop {
        let (mut socket, addr) = listener.accept().await?;
        println!("Connection from: {}", addr);

        let state = state.clone();

        tokio::spawn(async move {
            let (reader, mut writer) = socket.split();
            let mut lines = BufReader::new(reader).lines();

            writer.write_all(b"220 localhost LMTP ready\r\n").await.ok()?;

            let mut mail_from = String::new();
            let mut rcpt_to = String::new();
            let mut data = String::new();
            let mut in_data = false;

            while let Some(line) = lines.next_line().await.ok().flatten() {
                let line = line.trim_end();
                if in_data {
                    if line == "." {
                        in_data = false;

                        let status_code = process_email(state.clone(), mail_from.clone(), rcpt_to.clone(), data.clone()).await;

                        let response: &[u8] = match status_code {
                            StatusCode::OK => b"250 2.1.5 OK\r\n" as &[u8],
                            StatusCode::BAD_REQUEST => b"554 5.7.1 Message rejected\r\n" as &[u8],
                            StatusCode::FORBIDDEN => b"554 5.7.1 Message rejected\r\n" as &[u8],
                            StatusCode::SERVICE_UNAVAILABLE => b"451 4.3.0 Temporary failure\r\n" as &[u8],
                            _ => b"554 5.7.1 Message rejected\r\n" as &[u8],
                        };



                        writer.write_all(response).await.ok()?;

                        data.clear();
                    } else {
                        data.push_str(line);
                        data.push('\n');
                    }
                    continue;
                }

                if line.starts_with("LHLO") {
                    writer.write_all(b"250-localhost\r\n250-PIPELINING\r\n250 ENHANCEDSTATUSCODES\r\n").await.ok()?;
                } else if let Some(stripped) = line.strip_prefix("MAIL FROM:") {
                    mail_from = stripped.trim().to_string();
                    mail_from = mail_from.trim_start_matches('<').trim_end_matches('>').to_string();
                    writer.write_all(b"250 2.1.0 OK\r\n").await.ok()?;
                } else if let Some(stripped) = line.strip_prefix("RCPT TO:") {
                    rcpt_to = stripped.trim().to_string();
                    rcpt_to = rcpt_to.trim_start_matches('<').trim_end_matches('>').to_string();
                    writer.write_all(b"250 2.1.5 OK\r\n").await.ok()?;
                } else if line == "DATA" {
                    writer.write_all(b"354 End data with <CR><LF>.<CR><LF>\r\n").await.ok()?;
                    in_data = true;
                } else if line == "QUIT" {
                    writer.write_all(b"221 2.0.0 Bye\r\n").await.ok()?;
                    break;
                } else {
                    writer.write_all(b"502 5.5.2 Command not recognized\r\n").await.ok()?;
                }
            }

            Some(())
        });
    }
}


pub async fn process_email(
    state: Arc<AppState>,
    sender: String,
    recipient: String,
    data: String,
) -> StatusCode {
    println!("Processing email...");
    println!("From: {}", sender);
    println!("To: {}", recipient);
    println!("Data:\n{}", data);
    //
    // Early return for postmaster or invalid localpart
    let (user, tag) = match get_localpart(recipient.clone()) {
        Some(parts) => parts,
        None => return StatusCode::OK,
    };

    if let Some(tag) = tag {
        tracing::debug!("Email tag: {}", tag);
    }

    if let Ok(exists) = state.appservice.user_exists(&user).await {
        if exists {
            tracing::debug!("User exists: {}", user);
        } else {
            tracing::error!("User does not exist: {}", user);
            return StatusCode::OK;
        }
    } else {
        tracing::error!("Failed to check user existence");
        return StatusCode::SERVICE_UNAVAILABLE;
    }

    let message = match parse_message(&data).await {
        Ok(message) => message,
        Err(_) => {
            tracing::error!("Failed to parse email content");
            return StatusCode::BAD_REQUEST;
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
            tracing::error!("Failed to parse email content");
            return StatusCode::BAD_REQUEST;
        }
    };

    println!("Parsed email: {:#?}", email);

    // Let's upload the email to object storage
    let state_clone = state.clone();
    let raw = data.clone();
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
        

    let state_clone = state.clone();
    tokio::spawn(async move {
        tasks::process_email(state_clone, email, &user).await;
    });

    StatusCode::OK
}

