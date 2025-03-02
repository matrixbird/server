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

use serde_json::{json, Value};

use serde::{Serialize, Deserialize};

use crate::utils::get_localpart;

use crate::AppState;
use crate::hook::{
    EmailRequest,
    EmailBody,
    EmailContent,
    RelatesTo
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
        m_relates_to: None,
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
        m_relates_to: None,
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

    // send first matrix email
    if let Ok(body) = state.templates.render(
        "welcome_matrix.html",
        json!({
            "user": local_part,
        })
    ) {
        let subject = String::from("Welcome to Matrixbird");
        if let Ok(res) = state.appservice.send_to_inbox(
            room_id.clone(),
            subject,
            body.clone().to_string(),
            None
        ).await {
            tracing::info!("Welcome event sent - event ID: {:#?}", res);
        };
    }

    // send welcome email 
    if !state.development_mode() {

        let to = format!("{}@{}", local_part, state.config.email.domain);


        let sent = state.mail.send(
            &to,
            "Hello from Matrixbird",
            "welcome_email.html",
            json!({
                "user": local_part,
            }),
        );

        match sent.await {
            Ok(response) => {
                tracing::info!("Welcome email sent: {:#?}", response);
            }
            Err(e) => {
                tracing::warn!("Failed to send welcome email: {:#?}", e);
            }
        }

    } else {
        tracing::info!("Development mode: Skipping welcome email");
    }

    sleep(Duration::from_secs(3)).await;

    // send second matrix email
    if let Ok(body) = state.templates.render(
        "what_is_matrixbird.html",
        json!({})
    ) {
        let subject = String::from("What is Matrixbird?");
        if let Ok(res) = state.appservice.send_to_inbox(
            room_id,
            subject,
            body.clone().to_string(),
            None
        ).await {
            tracing::info!("Welcome event sent - event ID: {:#?}", res);
        };
    }

}

pub async fn process_reply(
    state: Arc<AppState>,
    event: Value,
) {

    let room_id = match event["room_id"].as_str() {
        Some(room_id) => room_id,
        None => return
    };

    let room_id = match OwnedRoomId::try_from(room_id) {
        Ok(room_id) => room_id,
        Err(e) => {
            tracing::error!("Failed to parse room ID: {}", e);
            return;
        }
    };

    let subject = match event["content"]["subject"].as_str() {
        Some(subject) => format!("Re: {}", subject),
        None => String::from("Re:"),
    };

    let relation = match event.pointer("/content/m.relates_to") {
        Some(relation) => relation,
        None => {
            tracing::error!("No relation found in event");
            return;
        }
    };

    let mut relation = match serde_json::from_value::<RelatesTo>(relation.clone()) {
        Ok(relation) => relation,
        Err(e) => {
            tracing::error!("Failed to parse relation: {}", e);
            return;
        }
    };


    if relation.event_id.is_none() || 
        relation.m_in_reply_to.is_none() ||
        relation.rel_type.is_none() {

        tracing::error!("No event ID found in relation");
        return;

    }

    let event_id = match event["event_id"].as_str() {
        Some(event_id) => event_id,
        None => return
    };

    relation.m_in_reply_to = Some(event_id.to_string());

    if let Ok(body) = state.templates.render(
        "auto_reply.html",
        json!({})
    ) {

        if let Ok(res) = state.appservice.send_to_inbox(
            room_id.clone(),
            subject,
            body.clone().to_string(),
            Some(relation)
        ).await {
            tracing::info!("Auto reply sent - event ID: {:#?}", res);
        };
    }
}
