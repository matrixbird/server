use axum::{
    extract::State,
    response::IntoResponse,
    Json,
};

use std::sync::Arc;

use serde_json::json;

use serde::Deserialize;

use crate::AppState;
use crate::error::AppserviceError;

use crate::tasks;

#[derive(Debug, Deserialize)]
pub struct SignupRequest {
    pub username: String,
    pub password: String,
    pub session: String,
    pub client_secret: String,
    pub invite_code: Option<String>,
}

pub async fn signup(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SignupRequest>,
) -> Result<impl IntoResponse, AppserviceError> {

    println!("signup request: {:?}", payload);

    if !state.development_mode() &&
        state.config.features.authentication.require_invite_code {

        match payload.invite_code.clone() {
            Some(code) => {

                println!("Invite code: {}", code);

                if let Ok(Some(email)) = state.db.invites.get_email(
                    &code
                ).await{
                    println!("Email is: {}", email);
                } else if code == state.config.general.invite_code.clone().unwrap_or("".to_string()) {
                    tracing::info!("Using default invite code");
                } else {
                    println!("Invite code not found");
                    return Ok(Json(json!({
                        "invited": false,
                        "error": "Invite code required"
                    })))
                }

            },
            None => {
                return Ok(Json(json!({
                    "invited": false,
                    "error": "Invite code required"
                })))
            }
        }
    }

    if let Ok(None) = state.session.get_code_session(
        payload.session.clone(),
    ).await {

        if state.config.features.authentication.require_verification {
            return Ok(Json(json!({
                "error": "not verified"
            })))
        }

    }

    let resp = state.auth.create_user(
        &payload.username,
        &payload.password
    ).await
        .map_err(|e| AppserviceError::HomeserverError(e.to_string()))?;

    println!("register response: {:?}", resp);

    // store user
    if let Ok(()) = state.db.users.create(
        resp.user_id.clone().as_str(),
        payload.username.clone().as_str()
    ).await{
        println!("created user");
    }

    // store access_token

    if let Some(token) = resp.access_token.clone() {
        if let Ok(()) = state.db.access_tokens.add(
            resp.user_id.clone().as_str(),
            token.clone().as_str()
        ).await{
            tracing::info!("Stored access token.");
        }
    }

    if let Ok(Some(request)) = state.session.get_code_session(
        payload.session.clone(),
    ).await {

        if let Ok(()) = state.db.users.add_email(
            resp.user_id.clone().as_str(),
            request.email.clone().as_str()
        ).await{
            println!("Added email to user");
        }

    }

    let username = payload.username.clone();
    let access_token = resp.access_token.clone();
    let user_id = resp.user_id.clone();
    let temp_state = state.clone();

    tokio::spawn(async move {
        if let Ok(inbox) = tasks::build_mailbox_rooms(
            temp_state,
            user_id,
            access_token,
            username,
        ).await {
            println!("Built mailbox rooms: {:?}", inbox);
        }
    });


    if let Some(access_token) = resp.access_token.clone() {
        if let Ok((id, _)) = state.session.create_session(
            resp.user_id.to_string(),
            access_token,
            resp.device_id.clone()
        ).await{

            return Ok(Json(json!({
                "session_id": id,
                "user_id": resp.user_id,
                "access_token": resp.access_token,
                "device_id": resp.device_id,
                "home_server": state.config.matrix.homeserver.clone(),
                "server_name": state.config.matrix.server_name.clone(),
            })));
            
        };
    }


    Ok(Json(json!({
        "error": "Could not register."
    })))
}

