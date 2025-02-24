use axum::{
    extract::State,
    response::IntoResponse,
    Json,
};

use std::sync::Arc;

use serde_json::json;

use chrono::{DateTime, Utc};

use serde::{Deserialize, Serialize};

//use ammonia::clean;



use crate::AppState;
use crate::error::AppserviceError;

use crate::utils::{get_localpart, get_email_subdomain};

use crate::tasks;

use ruma::{
    api::client::account::get_username_availability,
    events::macros::EventContent,
};

pub type HttpClient = ruma::client::http_client::HyperNativeTls;


#[derive(Clone, Debug, Deserialize, Serialize, EventContent)]
#[ruma_event(type = "matrixbird.email", kind = MessageLike)]
pub struct EmailContent {
    pub message_id: String,
    pub body: EmailBody,
    pub from: Address,
    pub subject: Option<String>,
    pub date: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<Attachment>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmailBody {
    pub text: Option<String>,
    pub html: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmailRequest {
    pub message_id: String,
    pub envelope_from: String,
    pub envelope_to: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_reply_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub references: Option<String>,
    pub from: Address,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sender: Option<Address>,
    pub to: Vec<Address>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cc: Option<Vec<Address>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bcc: Option<Vec<Address>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<Vec<Address>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    pub date: DateTime<Utc>,
    pub headers: Vec<Header>,
    pub content: Content,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<Attachment>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivered_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_path: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Address {
    pub address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Header {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Content {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub html: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Attachment {
    pub filename: String,
    pub path: String,
    pub mime_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
}


#[derive(Debug, Deserialize)]
pub struct InviteRequest {
    pub message_id: String,
    pub envelope_from: String,
    pub envelope_to: String,
    pub from: Address,
    pub to: Vec<Address>,
    pub subject: Option<String>,
    pub return_path: Option<String>,
}

use crate::utils::generate_invite_code;


pub async fn invite_hook(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<InviteRequest>,
) -> Result<impl IntoResponse, AppserviceError> {

    println!("INVITE email");
    println!("To: {:#?}", payload);

    let code = generate_invite_code();

    if let Ok(()) = state.db.add_invite(
        payload.envelope_from.clone().as_str(),
        code.clone().as_str()
    ).await{
        println!("Stored user invite");
    }

    if let Ok(res) = state.mailer.send_email_template(
        &payload.envelope_from,
        &code,
        "invite"
    ).await{
        println!("Email sent : {:#?}", res);
    }


    Ok(Json(json!({
        "action": "accept",
        "err": "none",
    })))
}


#[derive(Serialize)]
pub struct HookResponse {
    action: String,
}

impl HookResponse {
    fn accept() -> Self {
        Self {
            action: "accept".to_string(),
        }
    }

    fn reject() -> Self {
        Self {
            action: "reject".to_string(),
        }
    }
}


pub async fn hook(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<EmailRequest>,
) -> Json<HookResponse> {

    tracing::info!("Incoming email");
    tracing::info!("Message ID: {:?}", payload.message_id);
    tracing::info!("From: {:?}", payload.envelope_from);
    tracing::info!("To: {:?}", payload.envelope_to);
    tracing::info!("Subject: {:?}", payload.subject);
    tracing::info!("Date: {:?}", payload.date);

    // Early return for postmaster or invalid localpart
    let (user, tag) = match get_localpart(payload.envelope_to.clone()) {
        Some(parts) => parts,
        None => return Json(HookResponse::reject()),
    };

    if user == "postmaster" {
        return Json(HookResponse::accept())
    }

    if let Some(tag) = tag {
        tracing::debug!("Email tag: {}", tag);
    }

    let mxid = format!("@{}:{}", user, state.config.matrix.server_name);
    tracing::debug!("Processing email for MXID: {}", mxid);


    let client = ruma::Client::builder()
        .homeserver_url(state.config.matrix.homeserver.clone())
        //.access_token(Some(config.appservice.access_token.clone()))
        .build::<HttpClient>()
        .await.unwrap();

    let av = get_username_availability::v3::Request::new(
        user.clone(),
    );

    if let Ok(_) = client.send_request(av).await {
        tracing::error!("User does not exist: {}", mxid);

        // we'll accept emails for non-existing users if they come from out postmark saas, in order
        // to reduce hard bounces and getting flagged on their platform
        //
        if let Ok(subdomain) = get_email_subdomain(&payload.envelope_from) {
            if subdomain == "pm-bounces" && 
                user == "pm_bounces" {
                tracing::info!("Email from postmarkapp.com, accepting email for non-existing user");
                return Json(HookResponse::accept())
            }
        }


        return Json(HookResponse::reject())

    } else {
        tracing::info!("User exists: {}", mxid);
        let state_clone = state.clone();
        tokio::spawn(async move {
            tasks::process_email(state_clone, &payload, &user).await;
        });
        
        return Json(HookResponse::accept())
    }


    /*
    // Check if user exists
    match state.db.user_exists(&mxid).await {
        Ok(true) => {
            // Spawn async task to process email
            let state_clone = state.clone();
            tokio::spawn(async move {
                process_email(state_clone, &payload, &user).await;
            });
            
            return Json(HookResponse::accept())
        }
        _ => {
            tracing::error!("User does not exist: {}", mxid);
            return Json(HookResponse::reject())
        }
    }
    */
}
