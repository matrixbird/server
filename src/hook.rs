use axum::{
    extract::State,
    response::IntoResponse,
    Json,
};

use std::sync::Arc;

use serde_json::json;

use chrono::{DateTime, Utc};

use serde::{Deserialize, Serialize};

use ammonia::clean;



use crate::AppState;
use crate::error::AppserviceError;

use crate::utils::get_localpart;

use ruma::{
    RoomAliasId,
    api::client::account::get_username_availability,
    events::{
        AnyMessageLikeEventContent, 
        MessageLikeEventType,
        macros::EventContent,
    }
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
    //pub headers: Vec<Header>,
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

    if let Ok(res) = state.email.send_email_template(
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

async fn process_email(
    state: Arc<AppState>,
    payload: &EmailRequest,
    user: &str,
) {
    // Store email data first - independent operation
    let store_result = match serde_json::to_value(payload) {
        Ok(email_json) => {
            state.db.store_email_data(
                payload.message_id.as_str(),
                payload.envelope_from.as_str(),
                payload.envelope_to.as_str(),
                email_json,
            ).await
        }
        Err(e) => {
            tracing::error!("Failed to serialize email: {}", e);
            return;
        }
    };

    if let Err(e) = store_result {
        tracing::error!("Failed to store email: {}", e);
        return;
    }
    tracing::info!("Email stored successfully");

    // Try to send Matrix message
    let server_name = state.config.matrix.server_name.clone();
    let raw_alias = format!("#{}:{}", user, server_name);
    
    // Early return if we can't parse the alias
    let alias = match RoomAliasId::parse(&raw_alias) {
        Ok(alias) => alias,
        Err(e) => {
            tracing::error!("Failed to parse room alias: {}", e);
            return;
        }
    };

    // Early return if we can't get the room ID
    let room_id = match state.appservice.room_id_from_alias(alias).await {
        Some(id) => id,
        None => {
            tracing::error!("Failed to get room ID for alias");
            return;
        }
    };

    let ev_type = MessageLikeEventType::from("matrixbird.email.legacy");

    let safe_html = match payload.content.html.clone() {
        Some(html) => clean(&html),
        None => "".to_string(),
    };

    let email_body = EmailBody {
        text: payload.content.text.clone(),
        html: Some(safe_html),
    };
    let email_content = EmailContent {
        message_id: payload.message_id.clone(),
        body: email_body,
        from: payload.from.clone(),
        subject: payload.subject.clone(),
        date: payload.date.clone(),
        attachments: payload.attachments.clone(),

    };

    // Create and send the message
    let raw_event = match ruma::serde::Raw::new(&email_content) {
        Ok(raw) => raw.cast::<AnyMessageLikeEventContent>(),
        Err(e) => {
            tracing::error!("Failed to create raw event: {}", e);
            return;
        }
    };

    if let Err(e) = state.appservice.send_message(ev_type, room_id, raw_event).await {
        tracing::error!("Failed to send Matrix message: {}", e);
        return;
    }

    if let Err(e) = state.db.set_email_processed(&payload.message_id).await {
        tracing::error!("Failed to mark email as processed: {}", e);
        return;
    }

    tracing::info!("Email processed and message sent successfully");
}

pub async fn hook(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<EmailRequest>,
) -> Json<HookResponse> {
    tracing::info!("Incoming email: {:?}", payload);

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
        return Json(HookResponse::reject())
    } else {
        tracing::info!("User exists: {}", mxid);
        let state_clone = state.clone();
        tokio::spawn(async move {
            process_email(state_clone, &payload, &user).await;
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
