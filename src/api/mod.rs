use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};

use ruma::events::room::
    member::{RoomMemberEvent, MembershipState};
use ruma::OwnedRoomId;


use serde::{Serialize, Deserialize};
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::{info, warn};

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

#[derive(Debug)]
enum TransactionError {
    InvalidInput(String),
    DatabaseError(String),
    EmailError(String),
}

impl TransactionError {
    fn log(&self) {
        match self {
            Self::InvalidInput(msg) => warn!("Invalid input: {}", msg),
            Self::DatabaseError(msg) => warn!("Database error: {}", msg),
            Self::EmailError(msg) => warn!("Email error: {}", msg),
        }
    }
}

impl IntoResponse for TransactionError {
    fn into_response(self) -> Response {
        self.log();
        (StatusCode::OK, Json(json!({}))).into_response()
    }
}

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
            if let Err(e) =  state.db.store_event(
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
        tracing::info!("Event: {:#?}", event);

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
        if let Some(event_type) = event["type"].as_str() {

            if event_type.contains("matrixbird.email.standard") {
                tracing::info!("Outgoing standard email: {}", event_type);

                let reply_to = match event["content"]["to"].as_str() {
                    Some(to) => to,
                    None => ""
                };

                if reply_to == "" {
                    tracing::warn!("Missing reply_to");
                    continue;
                }

                let from = match event["content"]["from"]["address"].as_str() {
                    Some(from) => from,
                    None => ""
                };

                if from == "" {
                    tracing::warn!("Missing from");
                    continue;
                }

                let mut from = from.to_string();

                if state.development_mode() {
                    //replace domain part
                    let replaced = replace_email_domain(&from, state.config.email.domain.as_str());

                    from = replaced;
                }


                let message_id = match event["content"]["m.relates_to"]["matrixbird.in_reply_to"].as_str() {
                    Some(subject) => subject,
                    None => ""
                };

                if message_id == "" {
                    tracing::warn!("Missing message_id");
                    continue;
                }

                let subject = match event["content"]["subject"].as_str() {
                    Some(subject) => subject,
                    None => ""
                };

                let html = match event["content"]["body"]["html"].as_str() {
                    Some(html) => html,
                    None => ""
                };

                let text = match event["content"]["body"]["text"].as_str() {
                    Some(text) => text,
                    None => ""
                };


                let sent = state.mail.send_reply(
                    message_id,
                    reply_to,
                    from.to_string(),
                    subject,
                    text.to_string(),
                    html.to_string(),
                );

                match sent.await {
                    Ok(response) => {
                        tracing::info!("Matrix email reply sent: {:#?}", response);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to send email reply: {:#?}", e);
                    }
                }
            }


            if event_type.contains("matrixbird.email.reply") {
                tracing::info!("Outgoing matrix email: {}", event_type);

                let user_id = match event["content"]["to"].as_str() {
                    Some(to) => to,
                    None => ""
                };

                // Process auto-replies for @matrixbird user
                if user_id == state.appservice.user_id() {

                    let state_copy = state.clone();
                    let event_copy = event.clone();

                    tokio::spawn(async move {
                        tasks::process_reply(state_copy, event_copy).await;
                    });
                }


            }

        }

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



        let member_event = if let Ok(event) = serde_json::from_value::<RoomMemberEvent>(event.clone()) {
            event
        } else {
            continue;
        };


        let room_id = member_event.room_id().to_owned();
        let membership = member_event.membership().to_owned();
        let sender = member_event.sender().to_owned();


        let invited_user = member_event.state_key().to_owned();
        if invited_user != state.appservice.user_id() {
            info!("Ignoring event for user: {}", invited_user);
            continue;
        }

        match membership {
            MembershipState::Invite => {
                info!("Joining room: {}", room_id);

            if let Ok(room_id) =  state.appservice.join_room(room_id.clone()).await{

                if let Ok(room_type) = state.appservice.get_room_type(room_id.clone(), "INBOX".to_string()).await{
                    if room_type == "INBOX" {

                        let localpart = sender.localpart().to_owned();

                        let state_clone = state.clone();

                        // Send welcome emails and messages
                        tokio::spawn(async move {
                            tasks::send_welcome(
                                state_clone, 
                                &localpart,
                                room_id,
                            ).await;
                        });

                    }
                }



        };


            }
            _ => {}
        }

    }

    Ok(Json(json!({})))
}

