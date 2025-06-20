use axum::{
    extract::State,
    response::IntoResponse,
    Json,
};


//use tracing::{info, warn};

use std::sync::Arc;

use serde_json::{json, Value};

use serde::Deserialize;

use crate::AppState;
use crate::error::AppserviceError;

use ruma::api::client::{
    session::login,
    uiaa::UserIdentifier,
};

use crate::appservice::HttpClient;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub user: String,
    pub password: String,
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, AppserviceError> {

    tracing::info!("Login request: {:?}", payload);

    let resp = state.auth.login(
        &payload.user,
        &payload.password,
    ).await
    .map_err(|_| AppserviceError::AuthenticationError("Invalid credentials".to_string()))?;

    tracing::info!("Login response: {:?}", resp);

    // Store access token
    if let Ok(()) = state.db.access_tokens.add(
        resp.user_id.clone().as_str(),
        resp.access_token.clone().as_str()
    ).await{
        tracing::info!("Stored access token.");
    }


    if let Ok((id, session)) = state.session.create_session(
        resp.user_id.to_string(),
        resp.access_token,
        Some(resp.device_id.clone()),
    ).await{

        return Ok(Json(json!({
            "session_id": id,
            "access_token": session.access_token,
            "device_id": session.device_id,
            "home_server": state.config.matrix.homeserver.clone(),
            "server_name": state.config.matrix.server_name.clone(),
            "user_id": session.user_id,
        })));
        
    };


    Ok(Json(json!({
        "error": "Could not login."
    })))
}



pub async fn login_after_password_reset(
    state: Arc<AppState>,
    payload: LoginRequest,
) -> Result<Json<Value>, AppserviceError> {

    println!("Login request: {:?}", payload);

    let client = ruma::Client::builder()
        .homeserver_url(state.config.matrix.homeserver.clone())
        .build::<HttpClient>()
        .await
        .map_err(|_| AppserviceError::AuthenticationError("Invalid credentials".to_string()))?;


    let id = UserIdentifier::UserIdOrLocalpart(payload.user.clone());

    let pass = login::v3::Password::new(
        id,
        payload.password.clone()
    );

    let info = login::v3::LoginInfo::Password(pass);

    let resp = client
        .send_request(login::v3::Request::new(
            info
        ))
        .await
        .map_err(|_| AppserviceError::AuthenticationError("Invalid credentials".to_string()))?;

    println!("Login response: {:?}", resp);

    // Store access token
    if let Ok(()) = state.db.access_tokens.add(
        resp.user_id.clone().as_str(),
        resp.access_token.clone().as_str()
    ).await{
        tracing::info!("Stored access token.");
    }


    if let Ok((id, session)) = state.session.create_session(
        resp.user_id.to_string(),
        resp.access_token,
        Some(resp.device_id.clone()),
    ).await{

        return Ok(Json(json!({
            "session_id": id,
            "access_token": session.access_token,
            "device_id": session.device_id,
            "home_server": state.config.matrix.homeserver.clone(),
            "server_name": state.config.matrix.server_name.clone(),
            "user_id": session.user_id,
        })));
        
    };


    Ok(Json(json!({
        "error": "Could not login."
    })))
}



