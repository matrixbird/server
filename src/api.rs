use axum::{
    extract::State,
    http::StatusCode,
    Json,
};

use ruma::events::room::
    member::{RoomMemberEvent, MembershipState};


use serde_json::{Value, json};
use std::sync::Arc;
use tracing::info;

use crate::AppState;

use crate::tasks;

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

            let event_id = event_copy["event_id"].as_str();
            let room_id = event_copy["room_id"].as_str();
            let sender = event_copy["sender"].as_str();
            let event_type = event_copy["type"].as_str();

            match (event_id, room_id, sender, event_type) {
                (Some(event_id), Some(room_id), Some(sender), Some(event_type)) => {
                    if let Err(e) =  state_copy.db.store_event(
                        event_id,
                        room_id,
                        event_type,
                        sender,
                        event_copy.clone(),
                    ).await{
                        tracing::warn!("Failed to store event: {:#?}", e);
                    }

                },
                _ => {
                }
            }
        });

        if let Some(event_type) = event["type"].as_str() {

            if event_type == "matrixbird.email.legacy" {
                //tracing::info!("Outgoing legacy email: {}", event_type);
            }


            if event_type == "matrixbird.email.native" {
                tracing::info!("Outgoing native email: {}", event_type);

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

                state.appservice.join_room(room_id.clone()).await;

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
            _ => {}
        }

    }

    Ok(Json(json!({})))
}

