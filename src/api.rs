use axum::{
    extract::{State, OriginalUri},
    http::{
        Request, 
        Response, 
        StatusCode, 
        Uri, 
        HeaderValue, 
        header::AUTHORIZATION
    },
    response::IntoResponse,
    body::Body,
    Json,
    Extension,
};

use ruma::events::room::{
    member::{RoomMemberEvent, MembershipState},
    history_visibility::{RoomHistoryVisibilityEvent, HistoryVisibility},
};

use serde_json::{Value, json};
use std::sync::Arc;
use tracing::info;

use crate::AppState;
use crate::middleware::Data;

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

        // If auto-join is enabled, join rooms with world_readable history visibility
        if state.config.appservice.rules.auto_join {
            if let Ok(event) = serde_json::from_value::<RoomHistoryVisibilityEvent>(event.clone()) {
                match event.history_visibility() {
                    HistoryVisibility::WorldReadable => {
                        println!("History Visibility: World Readable");

                        let room_id = event.room_id().to_owned();
                        info!("Joining room: {}", room_id);
                        state.appservice.join_room(room_id).await;

                        return Ok(Json(json!({})))
                    }
                    _ => {}
                }
            }
        };

        let member_event = if let Ok(event) = serde_json::from_value::<RoomMemberEvent>(event.clone()) {
            event
        } else {
            continue;
        };

        //print!("Member Event: {:#?}", member_event);

        let room_id = member_event.room_id().to_owned();
        let membership = member_event.membership().to_owned();
        let server_name = member_event.room_id().server_name();

        match server_name {
            Some(server_name) => {

                let allowed = state.config.appservice.rules.federation_domain_whitelist.iter().any(|domain| {
                    server_name.as_str().ends_with(domain)
                });


                if server_name.as_str() != state.config.matrix.server_name && allowed {
                    // Ignore events for rooms on other servers, if configured to local homeserver
                    // users
                    if state.config.appservice.rules.invite_by_local_user {
                        info!("Ignoring event for room on different server: {}", server_name);
                        continue;
                    }
                }
            }
            None => {
                info!("Ignoring event for room with no server name");
                continue;
            }
        }

        // Ignore membership events for other users
        let invited_user = member_event.state_key().to_owned();
        if invited_user != state.appservice.user_id() {
            info!("Ignoring event for user: {}", invited_user);
            continue;
        }

        match membership {
            MembershipState::Invite => {
                info!("Joining room: {}", room_id);
                state.appservice.join_room(room_id.clone()).await;

                if let Some(body) = state.templates.get("welcome_matrix.html") {
                    if let Ok(res) = state.appservice.send_welcome_message(
                        room_id,
                        body.to_string(),
                    ).await {
                        println!("Send Welcome Message: {:#?}", res);
                    };
                }


            }
            MembershipState::Leave => {
                info!("Left room: {}", room_id);
            }
            MembershipState::Ban => {
                info!("Banned from room: {}", room_id);
                //state.appservice.leave_room(room_id).await;
            }
            _ => {}
        }


    }


    Ok(Json(json!({})))
}


pub async fn matrix_proxy(
    Extension(data): Extension<Data>,
    State(state): State<Arc<AppState>>,
    mut req: Request<Body>,
) -> Result<Response<Body>, StatusCode> {

    let mut path = if let Some(path) = req.extensions().get::<OriginalUri>() {
        path.0.path()
    } else {
        req.uri().path()
    };

    if let Some(mod_path) = data.modified_path.as_ref() {
        path = mod_path;
    }

    let path_query = req.uri().query().map(|q| format!("?{}", q)).unwrap_or_default();

    let homeserver = &state.config.matrix.homeserver;

    // add path query if path wasn't modified in middleware
    let uri = if data.modified_path.is_some() {
        format!("{}{}", homeserver, path)
    } else {
        format!("{}{}{}", homeserver, path, path_query)
    };

    *req.uri_mut() = Uri::try_from(uri).unwrap();

    let access_token = &state.config.appservice.access_token;

    let auth_value = HeaderValue::from_str(&format!("Bearer {}", access_token))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    req.headers_mut().insert(AUTHORIZATION, auth_value);

    let response = state.proxy
        .request(req)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
        .into_response();
        
    Ok(response)
}
