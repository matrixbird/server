use std::sync::Arc;
use tokio::time::{sleep, Duration};


use ruma::{
    RoomAliasId,
    OwnedRoomId,
    events::{
        AnyMessageLikeEventContent, 
        MessageLikeEventType,
    }
};

use crate::utils::get_localpart;

use crate::AppState;
use crate::hook::{
    EmailRequest,
    EmailBody,
    EmailContent,
};

pub async fn process_email(
    state: Arc<AppState>,
    payload: &EmailRequest,
    user: &str,
) {
    
    let store_result = match serde_json::to_value(payload) {
        Ok(email_json) => {
            state.db.store_email_data(
                payload.message_id.as_str(),
                payload.envelope_from.as_str(),
                payload.envelope_to.as_str(),
                email_json,
            ).await
        }
        Err(e) => {
            tracing::error!("Failed to serialize email: {}", e);
            return;
        }
    };

    if let Err(e) = store_result {
        tracing::error!("Failed to store email: {}", e);
        return;
    }
    tracing::info!("Email stored successfully");

    // Try to send Matrix message
    let server_name = state.config.matrix.server_name.clone();
    let raw_alias = format!("#{}:{}", user, server_name);
    
    // Early return if we can't parse the alias
    let alias = match RoomAliasId::parse(&raw_alias) {
        Ok(alias) => alias,
        Err(e) => {
            tracing::error!("Failed to parse room alias: {}", e);
            return;
        }
    };

    // Early return if we can't get the room ID
    let room_id = match state.appservice.room_id_from_alias(alias).await {
        Some(id) => id,
        None => {
            tracing::error!("Failed to get room ID for alias");
            return;
        }
    };

    let ev_type = MessageLikeEventType::from("matrixbird.email.legacy");

    /*
    let safe_html = match payload.content.html.clone() {
        Some(html) => clean(&html),
        None => "".to_string(),
    };
    */

    let email_body = EmailBody {
        text: payload.content.text.clone(),
        html: payload.content.html.clone(),
        //html: Some(safe_html),
    };
    let email_content = EmailContent {
        message_id: payload.message_id.clone(),
        body: email_body,
        from: payload.from.clone(),
        subject: payload.subject.clone(),
        date: payload.date.clone(),
        attachments: payload.attachments.clone(),

    };

    // Create and send the message
    let raw_event = match ruma::serde::Raw::new(&email_content) {
        Ok(raw) => raw.cast::<AnyMessageLikeEventContent>(),
        Err(e) => {
            tracing::error!("Failed to create raw event: {}", e);
            return;
        }
    };

    if let Err(e) = state.appservice.send_message(ev_type, room_id, raw_event).await {
        tracing::error!("Failed to send Matrix message: {}", e);
        return;
    }

    if let Err(e) = state.db.set_email_processed(&payload.message_id).await {
        tracing::error!("Failed to mark email as processed: {}", e);
        return;
    }

    tracing::info!("Email processed and message sent successfully");
}

pub async fn process_failed_emails(state: Arc<AppState>) {

    if let Ok(emails) = state.db.get_unprocessed_emails().await {
        for email in emails {

            let (user, _) = match get_localpart(email.envelope_to.clone()) {
                Some(parts) => parts,
                None => {
                    tracing::error!("Failed to get localpart from email: {:?}", email);
                    continue;
                }
            };

            println!("Processing email for user: {}", user);

            // deserialize the email json to EmailRequest 
            let payload: EmailRequest = match serde_json::from_value(email.email_json.clone()) {
                Ok(email) => email,
                Err(e) => {
                    tracing::error!("Failed to deserialize email: {}", e);
                    continue;
                }
            };

            let state_clone = state.clone();
            tokio::spawn(async move {
                process_failed_email(state_clone, &payload, &user).await;
            });

            sleep(Duration::from_secs(1)).await;

        }
    }

}

pub async fn process_failed_email(
    state: Arc<AppState>,
    payload: &EmailRequest,
    user: &str,
) {
    
    let server_name = state.config.matrix.server_name.clone();
    let raw_alias = format!("#{}:{}", user, server_name);
    
    let alias = match RoomAliasId::parse(&raw_alias) {
        Ok(alias) => alias,
        Err(e) => {
            tracing::error!("Failed to parse room alias: {}", e);
            return;
        }
    };

    let room_id = match state.appservice.room_id_from_alias(alias).await {
        Some(id) => id,
        None => {
            tracing::error!("Failed to get room ID for alias");
            return;
        }
    };

    let ev_type = MessageLikeEventType::from("matrixbird.email.legacy");

    /*
    let safe_html = match payload.content.html.clone() {
        Some(html) => clean(&html),
        None => "".to_string(),
    };
    */

    let email_body = EmailBody {
        text: payload.content.text.clone(),
        html: payload.content.html.clone(),
        //html: Some(safe_html),
    };
    let email_content = EmailContent {
        message_id: payload.message_id.clone(),
        body: email_body,
        from: payload.from.clone(),
        subject: payload.subject.clone(),
        date: payload.date.clone(),
        attachments: payload.attachments.clone(),

    };

    // Create and send the message
    let raw_event = match ruma::serde::Raw::new(&email_content) {
        Ok(raw) => raw.cast::<AnyMessageLikeEventContent>(),
        Err(e) => {
            tracing::error!("Failed to create raw event: {}", e);
            return;
        }
    };

    if let Err(e) = state.appservice.send_message(ev_type, room_id, raw_event).await {
        tracing::error!("Failed to send Matrix message: {}", e);
        return;
    }

    if let Err(e) = state.db.set_email_processed(&payload.message_id).await {
        tracing::error!("Failed to mark email as processed: {}", e);
        return;
    }

    tracing::info!("Email processed and message sent successfully");
}


pub async fn send_welcome(
    state: Arc<AppState>,
    local_part: &str,
    room_id: OwnedRoomId,
) {

    if let Some(body) = state.templates.get("welcome_matrix.html") {
        let subject = String::from("Welcome to Matrixbird");
        if let Ok(res) = state.appservice.send_welcome_message(
            room_id.clone(),
            subject,
            body.clone().to_string(),
        ).await {
            tracing::info!("Welcome event sent - event ID: {:#?}", res);
        };
    }

    if !state.development_mode() {

        if let Some(body) = state.templates.get("welcome_email.html") {

            let to = format!("{}@{}", local_part, state.config.email.domain);
            let res = state.mailer.send_email(
                &to,
                Some("welcome@matrixbird.org"),
                body,
                "Welcome to Matrixbird",
            ).await;

            match res {
                Ok(r) => {
                    tracing::info!("Welcome email sent: {:#?}", r);
                }
                Err(e) => {
                    tracing::warn!("Failed to send welcome email: {:#?}", e);
                }
            }
        }
    } else {
        tracing::info!("Development mode: Skipping welcome email");
    }


    sleep(Duration::from_secs(3)).await;

    if let Some(body) = state.templates.get("what_is_matrixbird.html") {
        let subject = String::from("What is Matrixbird?");
        if let Ok(res) = state.appservice.send_welcome_message(
            room_id,
            subject,
            body.clone().to_string(),
        ).await {
            tracing::info!("Welcome event sent - event ID: {:#?}", res);
        };
    }

}
