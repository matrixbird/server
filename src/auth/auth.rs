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

//use tracing::{info, warn};

use std::sync::Arc;

use serde_json::json;

use serde::Deserialize;

use crate::AppState;
use crate::error::AppserviceError;


use ruma::api::client::account::get_username_availability;

use crate::utils::generate_invite_code;

use crate::appservice::HttpClient;

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

    if let Ok(exists) = state.db.email_exists(
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


        let sent = state.mail.send(
            &payload.email,
            "Verification Code",
            "verification-code.html",
            serde_json::json!({
                "code": code,
            }),
        );

        match sent.await {
            Ok(response) => {
                println!("Email sent: {:?}", response);
            }
            Err(e) => {
                println!("Error sending email: {:?}", e);
            }
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
    //Path(device_id): Path<String>,
) -> Result<impl IntoResponse, AppserviceError> {

    let auth_header = headers.get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "));
        
    let session_id = auth_header.unwrap_or("");


    /*
    //let mut access_token: String = "".to_string();
    if let Ok(Some(session)) = state.session.get_session(
        session_id
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

        //access_token = session.access_token.clone();
        //
    }
        */

    if let Ok((valid, Some(session))) = state.session.validate_session(
        session_id,
    ).await{

        if valid {
            return Ok(Json(json!({
                "valid": true,
                "access_token": session.access_token,
                "device_id": session.device_id,
                "user_id": session.user_id,
            })));
        }

        
    };

    Ok(Json(json!({
        "valid": false
    })))
}

pub async fn revoke_session(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    //Path(device_id): Path<String>,
) -> Result<impl IntoResponse, AppserviceError> {

    let auth_header = headers.get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "));
        
    let session_id = auth_header.unwrap_or("");

    println!("Revoking session: {}", session_id);

    let _ = state.session.revoke_session(
        session_id,
    ).await;


    Ok(Json(json!({
    })))
}


pub async fn request_invite(
    State(state): State<Arc<AppState>>,
    Path(email): Path<String>,
) -> Result<impl IntoResponse, AppserviceError> {

    println!("Request invite for email: {}", email);

    tokio::spawn(async move {
        let reject = state.email_providers.reject(
            email.clone().as_str()
        ).await;

        if !reject {
            if let Ok(()) = state.db.add_invite(
                email.clone().as_str(),
                generate_invite_code().as_str()
            ).await{
                println!("Stored user invite");
            }
        }

    });


    Ok(Json(json!({
        "success": true
    })))
}

pub async fn validate_invite_code(
    State(state): State<Arc<AppState>>,
    Path(code): Path<String>,
) -> Result<impl IntoResponse, AppserviceError> {

    println!("Validating invite code: {}", code);

    if let Ok(Some(email)) = state.db.get_invite_code_email(
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

