use axum::{
    extract::State,
    response::IntoResponse,
    Json,
};
use tracing::{info, warn};

use std::sync::Arc;

use serde_json::json;

use serde::Deserialize;

use crate::AppState;
use crate::error::AppserviceError;

use ruma::{
    api::client::{
        session::login,
        uiaa::UserIdentifier,
    },
};


pub type HttpClient = ruma::client::http_client::HyperNativeTls;

use crate::cache::{
    store_verification_code,
    get_verification_code
};

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub user: String,
    pub password: String,
}


pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, AppserviceError> {

    println!("Login request: {:?}", payload);

    let client = ruma::Client::builder()
        .homeserver_url(state.config.matrix.homeserver.clone())
        //.access_token(Some(config.appservice.access_token.clone()))
        .build::<HttpClient>()
        .await.unwrap();

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
        .await.unwrap();

    println!("Login response: {:?}", resp);


    Ok(Json(json!({
        "user_id": resp.user_id,
        "access_token": resp.access_token,
        "device_id": resp.device_id,
    })))
}

#[derive(Debug, Deserialize)]
pub struct EmailRequest {
    pub email: String,
}

pub async fn verify_email(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<EmailRequest>,
) -> Result<impl IntoResponse, AppserviceError> {

    println!("email request: {:?}", payload);

    let mut redis_conn = state.cache.get_multiplexed_async_connection()
        .await;

    if let Ok(ref mut redis_conn) = redis_conn {

        if let Err(e) = store_verification_code(
            redis_conn, 
            payload.email.clone()
        ).await {
            warn!("Failed to store code : {}", e);
        } else {
            info!("Stored code for email: {}", payload.email);
        }

    }


    Ok(Json(json!({
        "sent": "yes"
    })))
}
