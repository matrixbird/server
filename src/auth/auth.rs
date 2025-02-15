use axum::{
    extract::{State, Path},
    response::IntoResponse,
    Json,
    //body::Body,
    http::{
        //Request, 
        //StatusCode, 
        //Uri, 
        //header::AUTHORIZATION,
        HeaderMap,
    },
};

use uuid::Uuid;

use crate::db::Queries;

//use tracing::{info, warn};

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
        //account::whoami,
        account::get_username_availability,
        room::create_room,
        //room::create_room::v3::CreationContent,
        session::login,
        uiaa::UserIdentifier,
        uiaa::AuthData,
        uiaa::Dummy,
    },
};

use crate::utils::{
    construct_matrix_id,
    //generate_magic_code,
    generate_invite_code
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


    let user_id = match construct_matrix_id(&payload.user, &state.config.matrix.server_name) {
        Some(id) => id,
        None => {
            println!("Invalid input");
            return Ok(Json(json!({
                "error": "Invalid input"
            })))
        }
    };

    let id = UserIdentifier::UserIdOrLocalpart(user_id);

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

    if let Ok(session) = state.session.create_session(
        resp.user_id.to_string(),
        resp.access_token,
        Some(resp.device_id.clone())
    ).await{

        return Ok(Json(json!({
            "session_id": session,
            "device_id": resp.device_id,
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
    pub session: String,
    pub client_secret: String,
    pub invite_code: Option<String>,
}


pub async fn signup(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SignupRequest>,
) -> Result<impl IntoResponse, AppserviceError> {

    println!("signup request: {:?}", payload);

    let mut invite_email: Option<String> = None;

    if state.config.features.require_invite_code {
        match payload.invite_code.clone() {
            Some(code) => {

                println!("Invite code: {}", code);

                if let Ok(Some(email)) = state.db.matrixbird.get_invite_code_email(
                    &code
                ).await{
                    println!("Email is: {}", email);
                    invite_email = Some(email);
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

        if state.config.features.require_verification {
            return Ok(Json(json!({
                "error": "not verified"
            })))
        }

    }


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


    if let Ok(Some(request)) = state.session.get_code_session(
        payload.session.clone(),
    ).await {

        if let Ok(()) = state.db.synapse.add_email(
            resp.user_id.clone().as_str(),
            request.email.clone().as_str()
        ).await{
            println!("Added email to user");
        }

    }

    match invite_email {
        Some(email) => {
            if let Ok(()) = state.db.synapse.add_email(
                resp.user_id.clone().as_str(),
                &email
            ).await{
                println!("Added email to user");

                if let Err(_) = state.db.matrixbird.activate_invite_code(
                    &email,
                    &payload.invite_code.clone().unwrap()
                ).await{
                    println!("Could not activate invite code");
                }
            }
        },
        None => {}
    }



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
                //"user_id": resp.user_id,
                //"access_token": resp.access_token,
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
    pub client_secret: String,
}

pub async fn verify_email(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<EmailRequest>,
) -> Result<impl IntoResponse, AppserviceError> {

    println!("email request: {:?}", payload);

    if let Ok(exists) = state.db.synapse.email_exists(
        payload.email.clone().as_str()
    ).await{
        if exists {
            let session_id = Uuid::new_v4().to_string();
            return Ok(Json(json!({
                "session": session_id
            })))
        }
    }

    let reject = state.email_providers.reject(
        payload.email.clone().as_str()
    ).await;

    if reject {
        return Ok(Json(json!({
            "reject": true,
            "error": "Email provider not allowed."
        })))
    }

    if let Ok((session, code)) = state.session.create_verification_code(
        payload.email.clone(),
        payload.client_secret.clone()
    ).await {

        if let Ok(res) = state.email.send_email_template(
            &payload.email,
            &code,
            "verification-code"
        ).await{
            println!("Email sent : {:#?}", res);
        }


        return Ok(Json(json!({
            "session": session
        })))
    }

    Ok(Json(json!({
        "error": "Could not send verification email."
    })))
}

#[derive(Debug, Deserialize)]
pub struct CodeRequest {
    pub email: String,
    pub client_secret: String,
    pub session: String,
    pub code: String,
}

pub async fn verify_code(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CodeRequest>,
) -> Result<impl IntoResponse, AppserviceError> {

    println!("code verification request: {:?}", payload);

    if let Ok(Some(request)) = state.session.get_code_session(
        payload.session.clone(),
    ).await {
        if request.code == payload.code && 
            request.client_secret == payload.client_secret &&
            request.email == payload.email {
            return Ok(Json(json!({
                "verified": true
            })))
        }
    }

    Ok(Json(json!({
        "error": "Could not verify code."
    })))
}

pub fn extract_token(header: &str) -> Option<&str> {
    if header.starts_with("Bearer ") {
        Some(header.trim_start_matches("Bearer ").trim())
    } else {
        None
    }
}

#[derive(Debug, Deserialize)]
pub struct SessionValidationRequest {
    pub session_id: String,
    pub device_id: String,
}

pub async fn validate_session(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(device_id): Path<String>,
) -> Result<impl IntoResponse, AppserviceError> {

    let auth_header = headers.get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "));
        
    let session_id = auth_header.unwrap_or("");

    //let mut access_token: String = "".to_string();

    if let Ok(Some(session)) = state.session.get_session(
        session_id
    ).await{
        println!("Session: {:?}", session);


        /*
        let client = ruma::Client::builder()
            .homeserver_url(state.config.matrix.homeserver.clone())
            .access_token(Some(session.access_token.clone()))
            .build::<HttpClient>()
            .await.unwrap();

        let whoami = client
            .send_request(whoami::v3::Request::new())
            .await.unwrap();

        if whoami.user_id.to_string() != session.user_id {
            return Ok(Json(json!({
                "valid": false
            })))
        }

        if whoami.device_id != session.device_id {
            return Ok(Json(json!({
                "valid": false
            })))
        }

        */
        //access_token = session.access_token.clone();

    }

    if let Ok((valid, Some(session))) = state.session.validate_session(
        session_id,
        &device_id,
    ).await{

        if valid {
            return Ok(Json(json!({
                "valid": true,
                "access_token": session.access_token,
                "user_id": session.user_id,
            })));
        }
        
    };

    Ok(Json(json!({
        "valid": false
    })))
}


pub async fn request_invite(
    State(state): State<Arc<AppState>>,
    Path(email): Path<String>,
) -> Result<impl IntoResponse, AppserviceError> {

    println!("Request invite for email: {}", email);

    let reject = state.email_providers.reject(
        email.clone().as_str()
    ).await;

    if reject {
        return Ok(Json(json!({
            "success": false,
            "error": "Email provider not allowed."
        })))
    }

    if let Ok(()) = state.db.matrixbird.add_invite(
        email.clone().as_str(),
        generate_invite_code().as_str()
    ).await{
        println!("Stored user invite");
    }


    Ok(Json(json!({
        "success": true
    })))
}

pub async fn validate_invite_code(
    State(state): State<Arc<AppState>>,
    Path(code): Path<String>,
) -> Result<impl IntoResponse, AppserviceError> {

    println!("Validating invite code: {}", code);

    if let Ok(Some(email)) = state.db.matrixbird.get_invite_code_email(
        &code
    ).await{
        println!("Email is: {}", email);
        return Ok(Json(json!({
            "valid": true,
            "email": email
        })))
    }

    Ok(Json(json!({
        "valid": false
    })))
}

