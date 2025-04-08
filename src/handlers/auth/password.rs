use axum::{
    extract::State,
    response::IntoResponse,
    Json,
};

use uuid::Uuid;


use std::sync::Arc;

use serde_json::json;

use serde::Deserialize;

use crate::AppState;
use crate::error::AppserviceError;


#[derive(Debug, Deserialize)]
pub struct PasswordResetRequest {
    pub username: String,
    pub client_secret: String,
}

pub async fn password_reset(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<PasswordResetRequest>,
) -> Result<impl IntoResponse, AppserviceError> {

    println!("email request: {:?}", payload);

    let user_id = format!(
        "@{}:{}", 
        payload.username.clone().to_lowercase(),
        state.config.matrix.server_name
    );

    println!("User ID: {}", user_id);

    let email = if let Ok(Some(email)) = state.db.users.get_email_from_user_id(
        &user_id
    ).await{
        email
    } else {
        println!("User not found");
        // Silently fail if the user doesn't exist
        // The user will wait expecting a verification code
        // But nothing will happen
        let session_id = Uuid::new_v4().to_string();
        return Ok(Json(json!({
            "session": session_id
        })))
    };


    if let Ok((session, code)) = state.session.create_verification_code(
        email.clone(),
        payload.client_secret.clone()
    ).await {

        let sent = state.mail.send(
            &email,
            "Verification Code",
            "verification_code.html",
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
pub struct PasswordResetCodeRequest {
    pub client_secret: String,
    pub session: String,
    pub code: String,
}

pub async fn verify_password_reset_code(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<PasswordResetCodeRequest>,
) -> Result<impl IntoResponse, AppserviceError> {

    println!("Password reset code verification request: {:?}", payload);

    if let Ok(Some(request)) = state.session.get_code_session(
        payload.session.clone(),
    ).await {
        if request.code == payload.code && 
            request.client_secret == payload.client_secret {
            return Ok(Json(json!({
                "verified": true
            })))
        }
    }

    Ok(Json(json!({
        "error": "Could not verify code."
    })))
}

#[derive(Debug, Deserialize)]
pub struct PasswordReset {
    pub client_secret: String,
    pub session: String,
    pub password: String,
}

pub async fn update_password(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<PasswordReset>,
) -> Result<impl IntoResponse, AppserviceError> {

    println!("Password reset request: {:?}", payload);

    if let Ok(Some(request)) = state.session.get_code_session(
        payload.session.clone(),
    ).await {
        if request.client_secret == payload.client_secret {


            if let Ok(()) = state.admin.reset_password(
                &payload.client_secret,
                &payload.password,
            ).await {
                return Ok(Json(json!({
                    "reset": true
                })))
            }

        }
    }

    Ok(Json(json!({
        "error": "Could not reset password."
    })))
}

