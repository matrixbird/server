pub mod user;

use std::sync::Arc;
use tokio::time::{sleep, Duration};
use std::collections::HashMap;


use ruma::{
    RoomAliasId,
    OwnedRoomId,
    OwnedUserId,
    events::{
        AnyMessageLikeEventContent, 
        MessageLikeEventType,
        InitialStateEvent,
        GlobalAccountDataEventType,
        AnyGlobalAccountDataEventContent,
        macros::EventContent,
    },
    api::client::room::create_room,
    api::client::config::set_global_account_data,
    api::client::profile::set_display_name,
};

use serde_json::{json, Value};

use serde::{Serialize, Deserialize};

use crate::utils::{get_localpart, get_mxid_localpart};

use crate::AppState;

use crate::email::{
    ParsedEmail,
    EmailBody,
    EmailContent,
    ReviewEmailContent,
    RelatesTo,
    ThreadMarkerContent
};

use crate::api::EmailReviewEvent;

use crate::appservice::HttpClient;



#[derive(Clone, Debug, Deserialize, Serialize, EventContent)]
#[ruma_event(type = "matrixbird.email.screen", kind = State, state_key_type = String)]
pub struct ScreenEmailsContent {
    pub screen: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, EventContent)]
#[ruma_event(type = "matrixbird.room.type", kind = State, state_key_type = String)]
pub struct RoomTypeContent {
    #[serde(rename = "type")]
    room_type: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, EventContent)]
#[ruma_event(type = "matrixbird.email.pending", kind = State, state_key_type = String)]
pub struct PendingEmailsContent {
    pub pending: Vec<EmailStateContent>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EmailStateContent {
    pub event_id: String,
    pub state: String,
}

pub async fn set_display_name(
    state: Arc<AppState>,
    user_id: OwnedUserId,
    name: String,
    access_token: Option<String>,
) -> Result<(), anyhow::Error> {

    let client = ruma::Client::builder()
        .homeserver_url(state.config.matrix.homeserver.clone())
        .access_token(access_token)
        .build::<HttpClient>()
        .await?;

    let req = set_display_name::v3::Request::new(
        user_id,
        Some(name),
    );

    client
        .send_request(req)
    .await?;

    Ok(())
}


pub async fn build_mailbox_rooms(
    state: Arc<AppState>,
    user_id: OwnedUserId,
    access_token: Option<String>,
    username: String,
) -> Result<(), anyhow::Error> {

    tokio::spawn(async move {

        let mut mailboxes = HashMap::new();

        let rooms = Vec::from([
            "INBOX",
            "DRAFTS",
            //"SCREEN",
            //"OUTBOX",
            //"SELF",
            //"TRASH",
            //"SPAM",
        ]);


        for room in rooms {
            let state_clone = state.clone();
            let access_token_clone = access_token.clone();

            if let Ok(room_id) = build_user_room(
                state_clone,
                username.clone(),
                access_token_clone,
                room.to_string()
            ).await {
                println!("Built user {} room: {:?}", room, room_id);
                
                mailboxes.insert(room.to_string(), room_id);
            }
        }

        println!("Mailboxes: {:?}", mailboxes);

        let raw_event = ruma::serde::Raw::new(&mailboxes)?;
        let raw = raw_event.cast::<AnyGlobalAccountDataEventContent>();

        let req = set_global_account_data::v3::Request::new_raw(
            user_id,
            GlobalAccountDataEventType::from("matrixbird.mailbox.rooms"),
            raw
        );

        let client = ruma::Client::builder()
            .homeserver_url(state.config.matrix.homeserver.clone())
            .access_token(access_token)
            .build::<HttpClient>()
            .await?;

        let resp = client.send_request(req).await?;

        tracing::info!("Mailbox rooms global account data response: {:?}", resp);

        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())

    });


    Ok(())
}

pub async fn build_user_room(
    state: Arc<AppState>,
    username: String,
    access_token: Option<String>,
    room_type: String,
) -> Result<String, anyhow::Error> {

    let client = ruma::Client::builder()
        .homeserver_url(state.config.matrix.homeserver.clone())
        .access_token(access_token)
        .build::<HttpClient>()
        .await?;

    let mut req = create_room::v3::Request::new();

    let rtc = RoomTypeContent {
        room_type: room_type.clone(),
    };

    let custom_state_event = InitialStateEvent {
        content: rtc,
        state_key: room_type.clone(),
    };

    let raw_event = custom_state_event.to_raw_any();

    req.initial_state = vec![raw_event];

    if room_type == "INBOX" {

        let sec = ScreenEmailsContent {
            screen: false,
        };

        let custom_state_event = InitialStateEvent {
            content: sec,
            state_key: "".to_string(),
        };

        let raw_event = custom_state_event.to_raw_any();

        req.initial_state.push(raw_event);

        let pec = PendingEmailsContent {
            pending: Vec::new(),
        };

        let custom_state_event = InitialStateEvent {
            content: pec,
            state_key: "".to_string(),
        };

        let raw_event = custom_state_event.to_raw_any();

        req.initial_state.push(raw_event);
    }


    req.name = Some(room_type.clone());
    req.preset = Some(create_room::v3::RoomPreset::TrustedPrivateChat);
    req.topic = Some(room_type.clone());

    req.room_alias_name = Some(format!("{}_{}", username, room_type));

    //if room_type == "INBOX" || room_type == "SCREEN" {
    if room_type == "INBOX" {
        let appservice_id = *state.appservice.user_id.clone();
        req.invite = vec![appservice_id];
    }

    let resp = client.send_request(req).await?;

    tracing::info!("{} room creation response: {:?}", room_type, resp);


    Ok(resp.room_id.to_string())
}


async fn build_event(
    state: Arc<AppState>,
    email: &ParsedEmail,
) -> Result<ruma::serde::Raw<AnyMessageLikeEventContent>, anyhow::Error>
{

    /*
    let safe_html = match email.content.html.clone() {
    Some(html) => clean(&html),
    None => "".to_string(),
    };


    let email_body = EmailBody {
        text: email.content.text.clone(),
        html: email.content.html.clone(),
        //html: Some(safe_html),
    };

    */

    const MAX_EVENT_SIZE_BYTES: usize = 20_000;

    let mut email_body = EmailBody {
        text: None,
        html: None,
        content_uri: None,
    };

    match (email.content.html.clone(), email.content.text.clone()) {
        (Some(html), Some(_)) | (Some(html), None) => {

            if html.len() > MAX_EVENT_SIZE_BYTES {
                if let Ok(uri) = state.appservice.upload_large_email(html).await {
                    email_body.content_uri = Some(uri);
                } else {
                    tracing::error!("Failed to upload large email content");
                }
            } else {
                email_body.html = Some(html);
            }
        },
        (None, Some(text)) => {
            if text.len() > MAX_EVENT_SIZE_BYTES {
                if let Ok(uri) = state.appservice.upload_large_email(text).await {
                    email_body.content_uri = Some(uri);
                } else {
                    tracing::error!("Failed to upload large email content");
                }
            } else {
                email_body.text = Some(text);
            }
        },
        _ => {}
    }

    let local_part = match get_localpart(email.recipient.clone()) {
        Some(parts) => parts.0,
        None => {
            tracing::error!("Failed to get localpart from email: {:?}", email);
            return Err(anyhow::Error::msg("Failed to get localpart from email"));
        }
    };

    let server_name = state.config.matrix.server_name.clone();
    let mxid = format!("@{}:{}", local_part, server_name);

    let email_content = EmailContent {
        message_id: email.message_id.clone(),
        body: email_body,
        from: email.from.clone(),
        recipients: vec![mxid],
        subject: email.subject.clone(),
        date: email.date.clone(),
        attachments: email.attachments.clone(),
        m_relates_to: None,
    };

    // Create and send the message
    let raw_event = match ruma::serde::Raw::new(&email_content) {
        Ok(raw) => raw.cast::<AnyMessageLikeEventContent>(),
        Err(e) => {
            tracing::error!("Failed to create raw event: {}", e);
            return Err(anyhow::Error::msg("Failed to create raw event"));
        }
    };

    Ok(raw_event)
}

pub async fn process_email(
    state: Arc<AppState>,
    email: ParsedEmail,
    user: &str,
) {

    let store_result = match serde_json::to_value(email.clone()) {
        Ok(email_json) => {
            state.db.emails.store(
                email.message_id.as_str(),
                email.sender.as_str(),
                email.recipient.as_str(),
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
    let raw_alias = format!("#{}_INBOX:{}", user, server_name);

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

    let address = email.sender.clone();

    let rule = match state.appservice.get_email_screen_rule(room_id.clone(), address.clone()).await {
        Ok(rule) => rule,
        Err(e) => {
            tracing::error!("Failed to get email allow rule: {}", e);
            "".to_string()
        }
    };

    let allow = rule == "allow";
    let reject = rule == "reject";
    //let pending = rule == "pending";
    let none = rule.is_empty();

    tracing::info!("Allowed to send to inbox?: {:?}", rule);

    if reject {
        tracing::info!("Email rejected by rule");
        return;
    }


    let ev_type = MessageLikeEventType::from("matrixbird.email.standard");
    // Create and send the message
    let raw_event = match build_event(state.clone(), &email).await {
        Ok(raw) => raw,
        Err(e) => {
            tracing::error!("Failed to create raw event: {}", e);
            return;
        }
    };

    let ev_id: String;

    match state.appservice.send_message(ev_type.clone(), room_id.clone(), raw_event.clone()).await {
        Ok(event_id) => {
            tracing::info!("Message sent successfully - event ID: {}", event_id);
            ev_id = event_id.clone();
            // set pending state event

            if none && (state.appservice.set_pending_email(room_id.clone(), event_id.clone()).await).is_ok() {
                tracing::info!("Pending email set successfully");
            }

            /*
            let pending = state.appservice.get_pending_email(room_id.clone()).await;
            match pending {
                Ok(Some(mut pending)) => {
                    pending.push(event_id.clone());
                    if let Ok(_) = state.appservice.set_pending_email(room_id.clone(), pending).await{
                        tracing::info!("Pending email set successfully");
                    }
                },
                Ok(None) => {
                    let pending = vec![event_id.clone()];
                    if let Ok(_) = state.appservice.set_pending_email(room_id.clone(), pending).await{
                        tracing::info!("Pending email set successfully");
                    }
                },
                Err(e) => {
                    tracing::error!("Failed to get pending emails: {}", e);
                }
            }
            */

            // set thread marker
            if allow {
                tracing::info!("Sending thread marker event...");

                let thread_marker = ThreadMarkerContent {
                    msgtype: "thread_marker".to_string(),
                    m_relates_to: RelatesTo {
                        event_id: Some(event_id.clone()),
                        m_in_reply_to: Some(event_id.clone()),
                        rel_type: Some("m.thread".to_string()),
                    },
                };

                let raw_event = match ruma::serde::Raw::new(&thread_marker) {
                    Ok(raw) => raw.cast::<AnyMessageLikeEventContent>(),
                    Err(e) => {
                        tracing::error!("Failed to create thread marker event: {}", e);
                        return;
                    }
                };

                if let Err(e) = state.appservice.send_message(
                    MessageLikeEventType::from("matrixbird.thread.marker"),
                    room_id,
                    raw_event,
                ).await {
                    tracing::error!("Failed to send thread marker event: {}", e);
                    return;
                }
            }


        },
        Err(e) => {
            tracing::error!("Failed to send Matrix message: {}", e);
            return;
        }
    }


    if let Err(e) = state.db.emails.set_processed(&email.message_id, ev_id).await {
        tracing::error!("Failed to mark email as processed: {}", e);
        return;
    }

    tracing::info!("Email processed and message sent successfully");
}

pub async fn process_failed_emails(state: Arc<AppState>) {

    if let Ok(emails) = state.db.emails.get_unprocessed().await {
        for email in emails {

            let (user, _) = match get_localpart(email.envelope_to.clone()) {
                Some(parts) => parts,
                None => {
                    tracing::error!("Failed to get localpart from email: {:?}", email);
                    continue;
                }
            };

            println!("Processing email for user: {}", user);

            // deserialize the email json to EmailRequest 
            let email: ParsedEmail = match serde_json::from_value(email.email_json.clone()) {
                Ok(email) => email,
                Err(e) => {
                    tracing::error!("Failed to deserialize email: {}", e);
                    continue;
                }
            };

            let state_clone = state.clone();
            tokio::spawn(async move {
                process_failed_email(state_clone, email, &user).await;
            });

            sleep(Duration::from_secs(1)).await;

        }
    }

}

pub async fn process_failed_email(
    state: Arc<AppState>,
    email: ParsedEmail,
    user: &str,
) {

    let server_name = state.config.matrix.server_name.clone();
    let raw_alias = format!("#{}:{}", user, server_name);

    let mxid = format!("@{}:{}", user, server_name);

    let alias = match RoomAliasId::parse(&raw_alias) {
        Ok(alias) => alias,
        Err(e) => {
            tracing::error!("Failed to parse room alias: {}", e);
            return;
        }
    };

    let room_id = match state.appservice.room_id_from_alias(alias).await {
        Some(id) => id,
        None => {
            tracing::error!("Failed to get room ID for alias");
            return;
        }
    };

    let ev_type = MessageLikeEventType::from("matrixbird.email.standard");

    /*
    let safe_html = match email.content.html.clone() {
    Some(html) => clean(&html),
    None => "".to_string(),
    };
    */

    let email_body = EmailBody {
        text: email.content.text.clone(),
        html: email.content.html.clone(),
        content_uri: None,
        //html: Some(safe_html),
    };

    let email_content = EmailContent {
        message_id: email.message_id.clone(),
        body: email_body,
        from: email.from.clone(),
        recipients: vec![mxid],
        subject: email.subject.clone(),
        date: email.date,
        attachments: email.attachments.clone(),
        m_relates_to: None,
    };

    // Create and send the message
    let raw_event = match ruma::serde::Raw::new(&email_content) {
        Ok(raw) => raw.cast::<AnyMessageLikeEventContent>(),
        Err(e) => {
            tracing::error!("Failed to create raw event: {}", e);
            return;
        }
    };


    let res = state.appservice.send_message(ev_type, room_id, raw_event).await;

    let ev_id: String = match res {
        Ok(event_id) => {
            tracing::info!("Message sent successfully - event ID: {}", event_id);
            event_id.clone()
        },
        Err(e) => {
            tracing::error!("Failed to send Matrix message: {}", e);
            return;
        }
    };



    if let Err(e) = state.db.emails.set_processed(&email.message_id, ev_id).await {
        tracing::error!("Failed to mark email as processed: {}", e);
        return;
    }

    tracing::info!("Email processed and message sent successfully");
}


pub async fn send_welcome(
    state: Arc<AppState>,
    sender: OwnedUserId,
    room_id: OwnedRoomId,
) {

    let local_part = sender.localpart().to_owned();
    // send first matrix email
    if let Ok(body) = state.templates.render(
        "welcome_matrix.html",
        json!({
            "user": local_part,
        })
    ) {
        let subject = String::from("Welcome to Matrixbird");
        if let Ok(event_id) = state.appservice.send_to_inbox(
            room_id.clone(),
            sender.clone(),
            subject,
            body.clone().to_string(),
            None,
            None
        ).await {
            tracing::info!("Welcome event sent - event ID: {:#?}", event_id);


            tracing::info!("Sending thread marker event...");

            let thread_marker = ThreadMarkerContent {
                msgtype: "thread_marker".to_string(),
                m_relates_to: RelatesTo {
                    event_id: Some(event_id.clone()),
                    m_in_reply_to: Some(event_id.clone()),
                    rel_type: Some("m.thread".to_string()),
                },
            };

            let raw_event = match ruma::serde::Raw::new(&thread_marker) {
                Ok(raw) => raw.cast::<AnyMessageLikeEventContent>(),
                Err(e) => {
                    tracing::error!("Failed to create thread marker event: {}", e);
                    return;
                }
            };

            if let Err(e) = state.appservice.send_message(
                MessageLikeEventType::from("matrixbird.thread.marker"),
                room_id.clone(),
                raw_event,
            ).await {
                tracing::error!("Failed to send thread marker event: {}", e);
                return;
            }

        };
    }

    // send welcome email 
    if state.config.email.settings.send_welcome_emails {

        let to = format!("{}@{}", local_part, state.config.email.incoming.domain);

        let sent = state.email.send(
            &to,
            "Hello from Matrixbird",
            "welcome_email.html",
            json!({
                "user": local_part,
            }),
        );

        match sent.await {
            Ok(response) => {
                tracing::info!("Welcome email sent: {:#?}", response);
            }
            Err(e) => {
                tracing::warn!("Failed to send welcome email: {:#?}", e);
            }
        }

    } else {
        tracing::info!("Development mode: Skipping welcome email");
    }

    //sleep(Duration::from_secs(3)).await;

    // send second matrix email
    if let Ok(body) = state.templates.render(
        "what_is_matrixbird.html",
        json!({})
    ) {
        let subject = String::from("What is Matrixbird?");
        if let Ok(event_id) = state.appservice.send_to_inbox(
            room_id.clone(),
            sender.clone(),
            subject,
            body.clone().to_string(),
            None,
            None
        ).await {
            tracing::info!("Welcome event sent - event ID: {:#?}", event_id);

            tracing::info!("Sending thread marker event...");

            let thread_marker = ThreadMarkerContent {
                msgtype: "thread_marker".to_string(),
                m_relates_to: RelatesTo {
                    event_id: Some(event_id.clone()),
                    m_in_reply_to: Some(event_id.clone()),
                    rel_type: Some("m.thread".to_string()),
                },
            };

            let raw_event = match ruma::serde::Raw::new(&thread_marker) {
                Ok(raw) => raw.cast::<AnyMessageLikeEventContent>(),
                Err(e) => {
                    tracing::error!("Failed to create thread marker event: {}", e);
                    return;
                }
            };

            if let Err(e) = state.appservice.send_message(
                MessageLikeEventType::from("matrixbird.thread.marker"),
                room_id.clone(),
                raw_event,
            ).await {
                tracing::error!("Failed to send thread marker event: {}", e);
            }

        };
    }

}

pub async fn process_reply(
    state: Arc<AppState>,
    event: Value,
) {

    let room_id = match event["room_id"].as_str() {
        Some(room_id) => room_id,
        None => return
    };

    let room_id = match OwnedRoomId::try_from(room_id) {
        Ok(room_id) => room_id,
        Err(e) => {
            tracing::error!("Failed to parse room ID: {}", e);
            return;
        }
    };

    let sender = match event["sender"].as_str() {
        Some(sender) => sender,
        None => return
    };

    let sender = match OwnedUserId::try_from(sender) {
        Ok(sender) => sender,
        Err(e) => {
            tracing::error!("Failed to parse sender: {}", e);
            return;
        }
    };


    let subject = match event["content"]["subject"].as_str() {
        Some(subject) => subject.to_string(),
        //Some(subject) => format!("Re: {}", subject),
        None => String::from("Re:"),
    };

    let relation = match event.pointer("/content/m.relates_to") {
        Some(relation) => relation,
        None => {
            tracing::error!("No relation found in event");
            return;
        }
    };

    let mut relation = match serde_json::from_value::<RelatesTo>(relation.clone()) {
        Ok(relation) => relation,
        Err(e) => {
            tracing::error!("Failed to parse relation: {}", e);
            return;
        }
    };


    if relation.event_id.is_none() || 
    relation.m_in_reply_to.is_none() ||
    relation.rel_type.is_none() {

        tracing::error!("No event ID found in relation");
        return;

    }

    let event_id = match event["event_id"].as_str() {
        Some(event_id) => event_id,
        None => return
    };

    relation.m_in_reply_to = Some(event_id.to_string());

    if let Ok(body) = state.templates.render(
        "auto_reply.html",
        json!({})
    ) {

        if let Ok(res) = state.appservice.send_to_inbox(
            room_id.clone(),
            sender,
            subject,
            body.clone().to_string(),
            Some(relation),
            Some("matrixbird.email.reply".to_string()),
        ).await {
            tracing::info!("Auto reply sent - event ID: {:#?}", res);
        };
    }
}

pub async fn send_email_review(
    state: Arc<AppState>,
    event: EmailReviewEvent,
    user: String,
) {
    println!("User: {}", user);

    let localpart = match get_mxid_localpart(&user) {
        Some(localpart) => localpart,
        None => {
            tracing::error!("Failed to get localpart from user ID");
            return;
        }
    };

    println!("Localpart: {}", localpart);

    // Try to send Matrix message
    let server_name = state.config.matrix.server_name.clone();
    let raw_alias = format!("#{}_INBOX:{}", localpart, server_name);

    // Early return if we can't parse the alias
    let alias = match RoomAliasId::parse(&raw_alias) {
        Ok(alias) => alias,
        Err(e) => {
            tracing::error!("Failed to parse room alias: {}", e);
            return;
        }
    };

    println!("Alias: {:?}", alias);

    // Early return if we can't get the room ID
    let room_id = match state.appservice.room_id_from_alias(alias).await {
        Some(id) => id,
        None => {
            tracing::error!("Failed to get room ID for alias");
            return;
        }
    };

    let ev_type = MessageLikeEventType::from("matrixbird.email.review");

    let email_body: EmailBody;

    if let Some(html) = event.content.body.html.clone() {
        email_body = EmailBody {
            text: None,
            html: Some(html),
            content_uri: None,
        };
    } else {
        email_body = EmailBody {
            text: event.content.body.text.clone(),
            html: None,
            content_uri: None,
        };
    }

    let email_content = ReviewEmailContent {
        body: email_body,
        from: event.content.from.clone(),
        subject: event.content.subject.clone(),
        to: event.content.to.clone(),
        invite_room_id: event.content.invite_room_id.clone(),
    };

    // Create and send the message
    let raw_event = match ruma::serde::Raw::new(&email_content) {
        Ok(raw) => raw.cast::<AnyMessageLikeEventContent>(),
        Err(e) => {
            tracing::error!("Failed to create raw event: {}", e);
            return;
        }
    };

    match state.appservice.send_message(ev_type, room_id.clone(), raw_event).await {
        Ok(event_id) => {
            tracing::info!("Email review sent to inbox - event ID: {}", event_id);
        },
        Err(e) => {
            tracing::error!("Failed to send email review event: {}", e);
        }
    }
}

