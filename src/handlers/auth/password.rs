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
        state.config.matrix.homeserver
    );

    let email = if let Ok(Some(email)) = state.db.users.get_email_from_user_id(
        &user_id
    ).await{
        email
    } else {
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

