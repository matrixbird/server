use axum::{
    extract::State,
    response::IntoResponse,
    Json,
};

use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use serde_json::json;

use chrono::{DateTime, Utc};

use serde::{Deserialize, Serialize};


use crate::AppState;
use crate::error::AppserviceError;

use crate::utils::{
    get_localpart,
};

use ruma::{
    RoomAliasId,
    events::{
        AnyMessageLikeEventContent, 
        MessageLikeEventType,
        //MessageLikeEventContent,
        macros::EventContent,
    }
};


#[derive(Clone, Debug, Deserialize, Serialize, EventContent)]
#[ruma_event(type = "matrixbird.email", kind = MessageLike)]
pub struct EmailContent {
    pub body: String,
}


#[derive(Debug, Clone)]
pub struct TransactionStore {
    current_id: Arc<RwLock<Option<String>>>,
}

impl TransactionStore {
    pub fn new() -> Self {
        Self {
            current_id: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn generate_transaction_id(&self) -> String {
        let transaction_id = Uuid::new_v4().to_string();
        let mut store = self.current_id.write().await;
        *store = Some(transaction_id.clone());
        transaction_id
    }

    pub async fn verify_and_remove_transaction(&self, transaction_id: &str) -> bool {
        let mut store = self.current_id.write().await;
        if let Some(stored_id) = store.as_ref() {
            if stored_id == transaction_id {
                *store = None;
                return true;
            }
        }
        false
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PingRequest {
    pub transaction_id: String,
}

pub async fn ping(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<PingRequest>,
) -> Result<impl IntoResponse, AppserviceError> {

    let txn_id = payload.transaction_id.clone();

    if !state.transaction_store.verify_and_remove_transaction(&txn_id).await {
        println!("Transaction ID does not match: {}", txn_id);
    }

    Ok(Json(json!({})))
}

#[derive(Debug, Deserialize)]
pub struct EmailRequest {
    pub message_id: String,
    pub envelope_from: String,
    pub envelope_to: String,
    pub in_reply_to: Option<String>,
    pub references: Option<String>,
    pub from: Address,
    pub sender: Option<Address>,
    pub to: Vec<Address>,
    pub cc: Option<Vec<Address>>,
    pub bcc: Option<Vec<Address>>,
    pub reply_to: Option<Vec<Address>>,
    pub subject: Option<String>,
    pub date: DateTime<Utc>,
    //pub headers: Vec<Header>,
    pub content: Content,
    pub attachments: Option<Vec<Attachment>>,
    pub delivered_to: Option<String>,
    pub return_path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Address {
    pub address: String,
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Header {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct Content {
    pub text: Option<String>,
    pub html: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Attachment {
    pub filename: String,
    pub path: String,
    pub mime_type: String,
    pub content_id: Option<String>,
    pub encoding: Option<String>,
}

pub async fn hook(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<EmailRequest>,
) -> Result<impl IntoResponse, AppserviceError> {

    println!("Incoming email");
    println!("To: {:#?}", payload);
    //println!("From: {:#?}", payload.from);
    //println!("Subject: {:#?}", payload.subject);
    //println!("Headers: {:#?}", payload.headers);
    //println!("Content: {:#?}", payload.content);
    //
    //


    if let Some((user, tag)) = get_localpart(&payload.envelope_to) {
        println!("localpart is: {}", user);

        if user == "postmaster" {
            return Ok(Json(json!({
                "action": "accept",
            })))
        }

        if let Some(tag) = tag {
            println!("Tag: {}", tag);
        }

        let mxid = format!("@{}:{}", user, state.config.matrix.server_name);
        println!("MXID: {}", mxid);

        let profile = state.appservice.get_profile(mxid).await;

        if let Some(profile) = profile {
            println!("Profile: {:#?}", profile);


            let server_name = state.config.matrix.server_name.clone();
            let raw_alias = format!("#{}:{}", user, server_name);
            println!("Raw Alias: {}", raw_alias);

            if let Ok(alias) = RoomAliasId::parse(&raw_alias) {
                let id = state.appservice.room_id_from_alias(alias).await;
                match id {
                    Some(id) => {
                        println!("Fetched Room ID: {:#?}", id);



                        let ev_type = MessageLikeEventType::from("matrixbird.email");



                        let em_cont = EmailContent{
                            body: payload.content.text.clone().unwrap_or_else(|| payload.content.html.clone().unwrap_or_else(|| "".to_string())),
                        };


                        let raw_event = ruma::serde::Raw::new(&em_cont)
                            .map_err(|_| AppserviceError::MatrixError("bad".to_string()))?;

                        let raw = raw_event.cast::<AnyMessageLikeEventContent>();



                        let re = state.appservice.send_message(
                            ev_type,
                            id,
                            raw
                        ).await;

                        println!("Send Message: {:#?}", re);

                    }
                    None => {}
                }
            }



        } else {
            return Ok(Json(json!({
                "action": "reject",
                "err": "user doesn't exist",
            })))
        }


    }

    Ok(Json(json!({
        "action": "accept",
        "err": "none",
    })))
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

use crate::db::Queries;

pub async fn invite_hook(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<InviteRequest>,
) -> Result<impl IntoResponse, AppserviceError> {

    println!("INVITE email");
    println!("To: {:#?}", payload);

    let code = generate_invite_code();

    if let Ok(()) = state.db.matrixbird.add_invite(
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

