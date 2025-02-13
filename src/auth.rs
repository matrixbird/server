use axum::{
    extract::{State, Path},
    response::IntoResponse,
    Json,
};
use tracing::{info, warn};

use std::sync::Arc;

use serde_json::json;

use serde::{Serialize, Deserialize};

use crate::AppState;
use crate::error::AppserviceError;

use ruma::{
    //OwnedRoomId,
    events::{
        //EmptyStateKey,
        InitialStateEvent,
        //AnyInitialStateEvent,
        //room::encryption::RoomEncryptionEventContent,
        //room::encryption::InitialRoomEncryptionEvent,
        macros::EventContent,
    },
    //room::RoomType as DefaultRoomType, 
    api::client::{
        account::register,
        account::whoami,
        account::get_username_availability,
        room::create_room,
        //room::create_room::v3::CreationContent,
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

    if let Ok(session) = state.session.create_session(
        resp.user_id.to_string(),
        resp.access_token,
        Some(resp.device_id.clone())
    ).await{

        return Ok(Json(json!({
            "session_id": session,
            "user_id": resp.user_id,
        })));
        
    };


    Ok(Json(json!({
        "error": "Could not login."
    })))
}

#[derive(Clone, Debug, Deserialize, Serialize, EventContent)]
#[ruma_event(type = "matrixbird.room.type", kind = State, state_key_type = String)]
pub struct RoomTypeContent {
    #[serde(rename = "type")]
    room_type: String,
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

    let temp_state = state.clone();

    tokio::spawn(async move {
        let client = ruma::Client::builder()
            .homeserver_url(temp_state.config.matrix.homeserver.clone())
            .access_token(access_token)
            .build::<HttpClient>()
            .await.unwrap();


        let mut req = create_room::v3::Request::new();


        /*
        let reec = RoomEncryptionEventContent::new(ruma::EventEncryptionAlgorithm::MegolmV1AesSha2);

        let iree = InitialRoomEncryptionEvent::new(reec);


        let aise = iree.to_raw_any();

        req.initial_state = vec![aise];
*/

        let rtc = RoomTypeContent {
            room_type: "INBOX".to_string()
        };

        let custom_state_event = InitialStateEvent {
            content: rtc,
            state_key: "inbox".to_string(), 
        };

        let raw_event = custom_state_event.to_raw_any();

        req.initial_state = vec![raw_event];

        /* DISABLE space
        let mut cc = CreationContent::new();
        cc.room_type = Some(DefaultRoomType::Space);
        let raw_cc = ruma::serde::Raw::new(&cc).unwrap();
        req.creation_content = Some(raw_cc);
        */

        req.name = Some("INBOX".to_string());

        req.room_alias_name = Some(username);

        req.preset = Some(create_room::v3::RoomPreset::TrustedPrivateChat);
        req.topic = Some("INBOX".to_string());

        let appservice_id = *temp_state.appservice.user_id.clone();

        req.invite = vec![appservice_id];

        if let Ok(res) = client.send_request(req).await {
            println!("room creation response: {:?}", res);
        }

    });

    if let Some(access_token) = resp.access_token.clone() {
        if let Ok(session) = state.session.create_session(
            resp.user_id.to_string(),
            access_token,
            resp.device_id.clone()
        ).await{

            return Ok(Json(json!({
                "session_id": session,
                "user_id": resp.user_id,
                "access_token": resp.access_token,
                "device_id": resp.device_id,
            })));
            
        };
    }


    Ok(Json(json!({
        "error": "Could not register."
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

#[derive(Debug, Deserialize)]
pub struct SessionValidationRequest {
    pub session_id: String,
    pub device_id: String,
}

pub async fn validate_session(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SessionValidationRequest>,
) -> Result<impl IntoResponse, AppserviceError> {

    /*
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
*/

    println!("session validation request: {:?}", payload);


    if let Ok(Some(session)) = state.session.get_session(
        &payload.session_id
    ).await{
        println!("Session: {:?}", session);


        let client = ruma::Client::builder()
            .homeserver_url(state.config.matrix.homeserver.clone())
            .access_token(Some(session.access_token.clone()))
            .build::<HttpClient>()
            .await.unwrap();

        let whoami = client
            .send_request(whoami::v3::Request::new())
            .await.unwrap();

        if whoami.user_id.to_string() != session.user {
            return Ok(Json(json!({
                "valid": false
            })))
        }

    }

    if let Ok(valid) = state.session.validate_session(
        &payload.session_id,
        &payload.device_id,
    ).await{

        if valid {
            return Ok(Json(json!({
                "valid": true,
            })));
        }
        
    };

    Ok(Json(json!({
        "valid": false
    })))
}

