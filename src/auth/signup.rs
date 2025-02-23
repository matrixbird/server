use axum::{
    extract::State,
    response::IntoResponse,
    Json,
};


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
        //session::login,
        //uiaa::UserIdentifier,
        uiaa::AuthData,
        uiaa::Dummy,
    },
};

use crate::appservice::HttpClient;

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

                if let Ok(Some(email)) = state.db.get_invite_code_email(
                    &code
                ).await{
                    println!("Email is: {}", email);
                    invite_email = Some(email);
                } else if code == state.config.authentication.invite_code.clone().unwrap_or("".to_string()) {
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

        if let Ok(()) = state.db.add_email(
            resp.user_id.clone().as_str(),
            request.email.clone().as_str()
        ).await{
            println!("Added email to user");
        }

    }

    match invite_email {
        Some(email) => {
            if let Ok(()) = state.db.add_email(
                resp.user_id.clone().as_str(),
                &email
            ).await{
                println!("Added email to user");

                if let Err(_) = state.db.activate_invite_code(
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

