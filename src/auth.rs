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

use ruma::{
    api::client::{
        session::login,
        uiaa::UserIdentifier,
    },
};


pub type HttpClient = ruma::client::http_client::HyperNativeTls;


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
