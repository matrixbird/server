mod service;
pub use service::*;

mod providers;
pub use providers::*;

mod incoming;
pub use incoming::*;

mod parse;
pub use parse::*;

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Email {
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParsedEmail {
    pub message_id: String,
    pub sender: String,
    pub recipient: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    pub date: DateTime<Utc>,
    pub content: Content,
}

