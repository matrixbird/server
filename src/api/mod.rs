use axum::{
    extract::State,
    http::StatusCode,
    Json,
};

use ruma::{
    OwnedRoomId,
    events::room::member::{RoomMemberEvent, MembershipState}
};

use ruma::events::macros::EventContent;


use serde::{Serialize, Deserialize};
use serde_json::{Value, json};
use std::sync::Arc;

use crate::AppState;

use crate::tasks;

use crate::utils::replace_email_domain;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct EmailReviewEvent {
    pub event_id: String,
    pub room_id: OwnedRoomId,
    pub sender: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub content: EmailReviewEventContent,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct EmailReviewEventContent {
    pub from: String,
    pub to: Vec<String>,
    pub subject: Option<String>,
    pub body: EmailBody,
    pub invite_room_id: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct EmailBody {
    pub text: Option<String>,
    pub html: Option<String>,
}

/*
fn deserialize_review_event(json: &str, user: String) -> Result<EmailReviewEvent, anyhow::Error> {
let event: EmailReviewEvent = serde_json::from_str(json)?;
if event.event_type == "matrixbird.email.review" && 
event.sender != user {
Ok(event)
} else {
Err(anyhow::anyhow!("Not an email review event"))
}
}
*/

async fn store_event_to_db(
    state: Arc<AppState>,
    event: Value,
) {

    let event_id = event["event_id"].as_str();
    let room_id = event["room_id"].as_str();
    let sender = event["sender"].as_str();
    let event_type = event["type"].as_str();

    let recipients = match event["content"]["recipients"].as_array() {
        Some(recipients) => {
            let mut recipients_vec = Vec::new();
            for recipient in recipients {
                if let Some(recipient) = recipient.as_str() {
                    recipients_vec.push(recipient);
                }
            }
            Some(recipients_vec)
        },
        None => None
    };

    let message_id = event["content"]["message_id"].as_str();

    let relates_to_event_id = event["content"]["m.relates_to"]["event_id"].as_str();
    let relates_to_in_reply_to = event["content"]["m.relates_to"]["m.in_reply_to"].as_str();
    let relates_to_rel_type = event["content"]["m.relates_to"]["rel_type"].as_str();

    match (event_id, room_id, sender, event_type) {
        (Some(event_id), Some(room_id), Some(sender), Some(event_type)) => {
            if let Err(e) =  state.db.events.store(
                event_id,
                room_id,
                event_type,
                sender,
                event.clone(),
                recipients,
                relates_to_event_id,
                relates_to_in_reply_to,
                relates_to_rel_type,
                message_id
            ).await{
                tracing::warn!("Failed to store event: {:#?}", e);
            }

        },
        _ => {
            tracing::warn!("Missing event fields");
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, EventContent)]
#[ruma_event(type = "matrixbird.room.type", kind = State, state_key_type = String)]
pub struct MatrixbirdRoomTypeEventContent {
    #[serde(rename = "type")]
    pub room_type: String,
}

pub async fn transactions(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {

    let events = match payload.get("events") {
        Some(Value::Array(events)) => events,
        Some(_) | None => {
            println!("Events is not an array");
            return Ok(Json(json!({})))
        }
    };

    for event in events {
        //println!("Event: {:#?}", event);
        //tracing::info!("Event: {:#?}", event);
        if cfg!(debug_assertions) {
            println!("Event: {:#?}", event);
        }

        let state_copy = state.clone();
        let event_copy = event.clone();

        tokio::spawn(async move {
            store_event_to_db(state_copy, event_copy).await;
        });

        /*
        if let Some(event_type) = event["type"].as_str() {

        if event_type == "matrixbird.email.review" {
        let to = match event["content"]["to"].as_str() {
        Some(to) => to,
        None => ""
        };
        }

        }
        */

        // Handle outgoing emails
        let state_copy = state.clone();
        let event_copy = event.clone();
        tokio::spawn(async move {
            process_outgoing(state_copy, event_copy).await;
        });



        /*
        // if this is a review event, send it to the recipient's inbox
        match deserialize_review_event(&event.to_string(), state.appservice.user_id()) {
        Ok(review_event) => {
        info!("Review event: {:#?}", review_event);

        let review_event_copy = review_event.clone();

        for recipient in review_event_copy.content.to {

        let state_copy = state.clone();
        let review_event_copy = review_event.clone();

        tokio::spawn(async move {
        tasks::send_email_review(
        state_copy,
        review_event_copy,
        recipient.to_string(),
        ).await;
        });
        }

        },
        Err(_) => {}
        }
        */

        // Join mailbox rooms of type INBOX
        /*
        if let Ok(event) = serde_json::from_value::<MatrixbirdRoomTypeEvent>(event.clone()) {
            tracing::info!("Matrixbird mailbox room event.");
            let room_id = event.room_id().to_owned();
            let room_type = event.state_key().to_owned();
            let sender = event.sender().to_owned();

            let is_inbox = room_type == "INBOX";

            if is_inbox {
                tracing::info!("Joining INBOX room: {}", room_id);

                if let Ok(room_id) =  state.appservice.join_room(room_id.clone()).await{

                    if let Ok(room_type) = state.appservice.get_room_type(room_id.clone(), "INBOX".to_string()).await{
                        if room_type == "INBOX" {

                            let state_clone = state.clone();

                            // Send welcome emails and messages
                            tokio::spawn(async move {
                                tasks::send_welcome(
                                    state_clone, 
                                    sender,
                                    room_id,
                                ).await;
                            });

                        }
                    }



                };


            }

        };
        */


        let member_event = if let Ok(event) = serde_json::from_value::<RoomMemberEvent>(event.clone()) {
            event
        } else {
            continue;
        };

        tracing::info!("Member event: {:#?}", member_event);


        let room_id = member_event.room_id().to_owned();
        let membership = member_event.membership().to_owned();
        let sender = member_event.sender().to_owned();

        if membership == MembershipState::Invite {

            // Auto-join rooms with user's access token
            let invited_user = member_event.state_key().to_owned();
            if invited_user != state.appservice.user_id() {
                continue;
            }

            tracing::info!("Joining room: {}", room_id);

            if let Ok(room_id) =  state.appservice.join_room(room_id.clone()).await{

                if let Ok(room_type) = state.appservice.get_room_type(room_id.clone(), "INBOX".to_string()).await{
                    if room_type == "INBOX" {

                        let state_clone = state.clone();

                        // Send welcome emails and messages
                        tokio::spawn(async move {
                            tasks::send_welcome(
                                state_clone, 
                                sender,
                                room_id,
                            ).await;
                        });

                    }
                }


            };

        }

    }

    Ok(Json(json!({})))
}

async fn process_outgoing(
    state: Arc<AppState>,
    event: Value,
) {

    let event_type = match event["type"].as_str() {
        Some(event_type) => event_type,
        None => {
            tracing::warn!("Missing event type");
            return;
        }
    };

    if event_type.contains("matrixbird.email.standard") {
        process_standard_email(state, event).await;
    } else if event_type.contains("matrixbird.email.reply") {
        process_email_reply(state, event).await;
    }

}

async fn process_standard_email(state: Arc<AppState>, event: Value) {
    tracing::info!("Outgoing standard email: {}", event["type"].as_str().unwrap_or_default());

    let reply_to = match event["content"]["to"].as_str() {
        Some(to) if !to.is_empty() => to,
        _ => {
            tracing::warn!("Missing reply_to");
            return;
        }
    };

    let from = match event["content"]["from"]["address"].as_str() {
        Some(from) if !from.is_empty() => from,
        _ => {
            tracing::warn!("Missing from");
            return;
        }
    };

    let mut from = from.to_string();

    if state.development_mode() {
        // Replace domain part
        from = replace_email_domain(&from, state.config.email.incoming.domain.as_str());
    }

    let message_id = match event["content"]["m.relates_to"]["matrixbird.in_reply_to"].as_str() {
        Some(id) if !id.is_empty() => id,
        _ => {
            tracing::warn!("Missing message_id");
            return;
        }
    };

    let subject = event["content"]["subject"].as_str().unwrap_or_default();
    let html = event["content"]["body"]["html"].as_str().unwrap_or_default();
    let text = event["content"]["body"]["text"].as_str().unwrap_or_default();

    match state
        .email
        .send_reply(
            message_id,
            reply_to,
            from,
            subject,
            text.to_string(),
            html.to_string(),
        )
        .await
    {
        Ok(response) => {
            tracing::info!("Matrix email reply sent: {:#?}", response);
        }
        Err(e) => {
            tracing::warn!("Failed to send email reply: {:#?}", e);
        }
    }
}

async fn process_email_reply(state: Arc<AppState>, event: Value) {
    tracing::info!("Outgoing matrix email: {}", event["type"].as_str().unwrap_or_default());

    let recipients: Vec<String> = event["content"]["recipients"]
        .as_array()
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    if recipients.iter().any(|r| *r == state.appservice.user_id()) {
        tasks::process_reply(state.clone(), event.clone()).await;
    }
}
