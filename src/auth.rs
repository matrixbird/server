use axum::{
    extract::{State, Path},
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
        account::register,
        account::get_username_availability,
        room::create_room,
        session::login,
        uiaa::UserIdentifier,
        uiaa::AuthData,
        uiaa::Dummy,
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
pub struct SignupRequest {
    pub username: String,
    pub password: String,
}


pub async fn signup(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SignupRequest>,
) -> Result<impl IntoResponse, AppserviceError> {

    println!("signup request: {:?}", payload);

    let client = ruma::Client::builder()
        .homeserver_url(state.config.matrix.homeserver.clone())
        //.access_token(Some(config.appservice.access_token.clone()))
        .build::<HttpClient>()
        .await.unwrap();

    let av = get_username_availability::v3::Request::new(
        payload.username.clone()
    );

    if let Err(res) = client.send_request(av).await {
        println!("username availability response: {:?}", res);
        return Ok(Json(json!({
            "available": false
        })))
    }


    let mut req = register::v3::Request::new();

    req.username = Some(payload.username.clone());
    req.password = Some(payload.password.clone());

    let dum = Dummy::new();

    let authdata = AuthData::Dummy(dum);

    req.auth = Some(authdata);

    let resp = client
        .send_request(req)
        .await.unwrap();

    println!("register response: {:?}", resp);

    let username = payload.username.clone();
    let access_token = resp.access_token.clone();

    tokio::spawn(async move {
        let client = ruma::Client::builder()
            .homeserver_url(state.config.matrix.homeserver.clone())
            .access_token(access_token)
            .build::<HttpClient>()
            .await.unwrap();


        let mut req = create_room::v3::Request::new();

        req.name = Some("Test Room".to_string());

        let alias = format!("{}_{}", username, "INBOX");
        req.room_alias_name = Some(alias);

        req.preset = Some(create_room::v3::RoomPreset::TrustedPrivateChat);
        req.topic = Some("INBOX".to_string());

        let appservice_id = *state.appservice.user_id.clone();

        req.invite = vec![appservice_id];

        if let Ok(res) = client.send_request(req).await {
            println!("room creation response: {:?}", res);
        }

    });




    Ok(Json(json!({
        "user_id": resp.user_id,
        "access_token": resp.access_token,
        "device_id": resp.device_id,
    })))
}

pub async fn username_available(
    State(state): State<Arc<AppState>>,
    Path(username): Path<String>,
) -> Result<impl IntoResponse, AppserviceError> {

    let client = ruma::Client::builder()
        .homeserver_url(state.config.matrix.homeserver.clone())
        .build::<HttpClient>()
        .await.unwrap();

    let av = get_username_availability::v3::Request::new(
        username.clone()
    );

    if let Ok(res) = client.send_request(av).await {

        println!("username availability response: {:?}", res);

        return Ok(Json(json!({
            "available": res.available
        })))
    }

    Ok(Json(json!({
        "available": false
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
