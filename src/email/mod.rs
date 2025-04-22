mod service;
pub use service::*;

mod providers;
pub use providers::*;

mod incoming;
pub use incoming::*;

mod parse;
pub use parse::*;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParsedEmail {
    pub message_id: String,
    pub sender: String,
    pub recipient: String,
    pub from: Address,
    pub to: Vec<Address>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_reply_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    pub date: String,
    pub content: Content,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<Attachment>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Address {
    pub address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
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
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ThreadMarkerContent {
    pub msgtype: String,
    #[serde(rename = "m.relates_to")]
    pub m_relates_to: RelatesTo,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EmailContent {
    pub message_id: String,
    pub body: EmailBody,
    pub from: Address,
    pub recipients: Vec<String>,
    pub subject: Option<String>,
    pub date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<Attachment>>,
    #[serde(rename = "m.relates_to")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub m_relates_to: Option<RelatesTo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmailBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub html: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_uri: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RelatesTo {
    pub event_id: Option<String>,
    #[serde(rename = "m.in_reply_to")]
    pub m_in_reply_to: Option<String>,
    pub rel_type: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ReviewEmailContent {
    pub body: EmailBody,
    pub from: String,
    pub subject: Option<String>,
    pub to: Vec<String>,
    pub invite_room_id: String,
}

