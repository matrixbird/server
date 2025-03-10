use crate::config::Config;
use chrono::Utc;

use serde::{Serialize, Deserialize};

use ruma::{
    OwnedRoomId,
    OwnedEventId,
    OwnedUserId,
    OwnedTransactionId,
    TransactionId,  
    UserId,
    api::client::{
        appservice::request_ping,
        alias::get_alias,
        account::whoami, 
        membership::joined_rooms, 
        message::send_message_event,
        state::{
            get_state_events, 
            get_state_events_for_key,
            send_state_event,
        },
        room::get_room_event,
        membership::{
            join_room_by_id, 
            leave_room
        },
        profile::get_profile,
        config::set_global_account_data,
    },
    events::{
        AnyMessageLikeEventContent, 
        MessageLikeEventType,
        AnyTimelineEvent,
        AnyStateEvent, 
        AnyStateEventContent,
        StateEventType,
        GlobalAccountDataEventType,
        AnyGlobalAccountDataEventContent,
    }
};

use uuid::Uuid;

use anyhow;

use crate::tasks::{EmailStateContent, PendingEmailsContent};

use crate::hook::{EmailBody, EmailContent, Address, RelatesTo};

pub type HttpClient = ruma::client::http_client::HyperNativeTls;

#[derive(Clone)]
pub struct AppService {
    client: ruma::Client<HttpClient>,
    pub appservice_id: String,
    pub user_id: Box<OwnedUserId>,
}

pub type RoomState = Vec<ruma::serde::Raw<AnyStateEvent>>;


impl AppService {
    pub async fn new(config: &Config) -> Result<Self, anyhow::Error> {

        let client = ruma::Client::builder()
            .homeserver_url(config.matrix.homeserver.clone())
            .access_token(Some(config.appservice.access_token.clone()))
            .build::<HttpClient>()
            .await?;

        let user_id = UserId::parse(&format!("@{}:{}", config.appservice.sender_localpart, config.matrix.server_name))?;

        let whoami = client
            .send_request(whoami::v3::Request::new())
            .await;

        if let Err(_) = whoami {
            tracing::error!("Failed to authenticate with homeserver");
            std::process::exit(1);
        }

        Ok(Self { 
            client, 
            appservice_id: config.appservice.id.clone(),
            user_id: Box::new(user_id)
        })
    }

    pub async fn ping_homeserver(&self, id: String) -> Result<request_ping::v1::Response, anyhow::Error> {

        let mut req = request_ping::v1::Request::new(
            self.appservice_id.to_string()
        );

        req.transaction_id = Some(OwnedTransactionId::try_from(id)?);

        let response = self.client
            .send_request(req)
            .await?;
        Ok(response)
    }

    pub fn user_id(&self) -> String {
        self.user_id.to_string()
    }

    pub async fn whoami(&self) -> Result<whoami::v3::Response, anyhow::Error> {
        let r = self.client
            .send_request(whoami::v3::Request::new())
            .await?;
        Ok(r)
    }

    pub async fn join_room(&self, room_id: OwnedRoomId) -> Result<OwnedRoomId, anyhow::Error> {

        let jr = self.client
            .send_request(join_room_by_id::v3::Request::new(
                room_id
            ))
            .await?;

        println!("Join room: {:#?}", jr);

        Ok(jr.room_id)
    }

    pub async fn get_room_type(&self, room_id: OwnedRoomId, room_type: String) -> Result<String, anyhow::Error> {

        let jr = self.client
            .send_request(get_state_events_for_key::v3::Request::new(
                room_id,
                StateEventType::from("matrixbird.room.type"),
                room_type
            ))
            .await?;

        if let Ok(room_type) = jr.content.get_field::<String>("type") {
            match room_type {
                Some(rt) => Ok(rt),
                None => Err(anyhow::anyhow!("Room type not found"))
            }
        } else {
            Err(anyhow::anyhow!("Room type not found"))
        }

    }

    pub async fn get_pending_email(&self, room_id: OwnedRoomId) -> Result<Option<Vec<EmailStateContent>>, anyhow::Error> {

        let jr = self.client
            .send_request(get_state_events_for_key::v3::Request::new(
                room_id,
                StateEventType::from("matrixbird.email.pending"),
                "".to_string()
            ))
            .await?;

        if let Ok(pending) = jr.content.get_field::<Vec<EmailStateContent>>("pending") {
            match pending {
                Some(pending) => {
                    tracing::info!("Pending: {:#?}", pending);
                    Ok(Some(pending))
                },
                None => Ok(Some(vec![]))
            }
        } else {
            Err(anyhow::anyhow!("Pending emails not found"))
        }

    }

    pub async fn set_pending_email(&self, room_id: OwnedRoomId, event_id: String) -> Result<OwnedEventId, anyhow::Error> {

        let res = self.get_pending_email(room_id.clone()).await?;

        let mut pending = match res {
            Some(p) => p,
            None => vec![]
        };


        let state = EmailStateContent {
            event_id,
            state: "pending".to_string(),
        };

        pending.push(state);

        let content = PendingEmailsContent {
            pending,
        };

        let raw_event = ruma::serde::Raw::new(&content)?;
        let raw = raw_event.cast::<AnyStateEventContent>();

        let req = send_state_event::v3::Request::new_raw(
            room_id,
            StateEventType::from("matrixbird.email.pending"),
            "".to_string(),
            raw
        );

        let res = self.client
            .send_request(req)
            .await?;

        Ok(res.event_id)

    }


    pub async fn get_email_screen_rule(&self, room_id: OwnedRoomId, address: String) -> Result<String, anyhow::Error> {

        let jr = self.client
            .send_request(get_state_events_for_key::v3::Request::new(
                room_id,
                StateEventType::from("matrixbird.email.rule"),
                address
            ))
            .await?;

        if let Ok(screen_rule) = jr.content.get_field::<String>("rule") {
            match screen_rule {
                Some(rule) => {
                    tracing::info!("Allow: {:#?}", rule);
                    Ok(rule)
                }
                None => Ok("".to_string())
            }
        } else {
            Err(anyhow::anyhow!("Screen rule not found"))
        }

    }

    pub async fn set_email_screen_rule(&self, room_id: OwnedRoomId, address: String, rule: String, event_id: String) -> Result<OwnedEventId, anyhow::Error> {


        #[derive(Serialize, Deserialize)]
        struct Content {
            rule: String,
            event_id: String,
        }

        let content = Content {
            rule,
            event_id,
        };

        let raw_event = ruma::serde::Raw::new(&content)?;
        let raw = raw_event.cast::<AnyStateEventContent>();

        let req = send_state_event::v3::Request::new_raw(
            room_id,
            StateEventType::from("matrixbird.email.rule"),
            address,
            raw
        );

        let res = self.client
            .send_request(req)
            .await?;

        Ok(res.event_id)

    }

    pub async fn set_state_event(&self, room_id: OwnedRoomId, data_type: String, state_key: String, content: String) -> Result<OwnedEventId, anyhow::Error> {

        let raw_event = ruma::serde::Raw::new(&content)?;
        let raw = raw_event.cast::<AnyStateEventContent>();

        let req = send_state_event::v3::Request::new_raw(
            room_id,
            StateEventType::from(data_type),
            state_key,
            raw
        );

        let res = self.client
            .send_request(req)
            .await?;

        Ok(res.event_id)
    }


    pub async fn set_joined_room_account_data(&self, data_type: String, content: String) -> Result<(), anyhow::Error> {

        let raw_event = ruma::serde::Raw::new(&content)?;
        let raw = raw_event.cast::<AnyGlobalAccountDataEventContent>();

        let req = set_global_account_data::v3::Request::new_raw(
            *self.user_id.clone(),
            GlobalAccountDataEventType::from(data_type),
            raw
        );

        let _ = self.client
            .send_request(req)
            .await?;

        Ok(())
    }

    pub async fn set_global_account_data(&self, data_type: String, content: ruma::serde::Raw<AnyGlobalAccountDataEventContent>) -> Result<(), anyhow::Error> {

        let req = set_global_account_data::v3::Request::new_raw(
            *self.user_id.clone(),
            GlobalAccountDataEventType::from(data_type),
            content
        );

        let _ = self.client
            .send_request(req)
            .await?;

        Ok(())
    }

    pub async fn has_joined_room(&self, room_id: OwnedRoomId) -> bool {

        let jr = self.client
            .send_request(get_state_events_for_key::v3::Request::new(
                room_id,
                StateEventType::RoomMember,
                self.user_id()
            ))
            .await 
            .ok();

        jr.is_some()
    }


    pub async fn get_room_state(&self, room_id: OwnedRoomId) ->
    Option<RoomState> {

        let state = self.client
            .send_request(get_state_events::v3::Request::new(
                room_id,
            ))
            .await
            .ok()?;

        Some(state.room_state)
    }

    pub async fn leave_room(&self, room_id: OwnedRoomId) {

        let jr = self.client
            .send_request(leave_room::v3::Request::new(
                room_id
            ))
            .await
            .ok();
        println!("Left room: {:#?}", jr);
    }

    pub async fn joined_rooms(&self) -> Option<Vec<ruma::OwnedRoomId>> {
        let jr = self.client
            .send_request(joined_rooms::v3::Request::new())
            .await
            .ok()?;

        Some(jr.joined_rooms)
    }

    pub async fn room_id_from_alias(&self, room_alias: ruma::OwnedRoomAliasId) -> Option<ruma::OwnedRoomId> {

        let room_id = self.client
            .send_request(get_alias::v3::Request::new(
                room_alias,
            ))
            .await
            .ok()?;

        Some(room_id.room_id)
    }


    pub async fn get_room_event(&self, room_id: OwnedRoomId, event_id: OwnedEventId) -> Option<ruma::serde::Raw<AnyTimelineEvent>> {

        let event = self.client
            .send_request(get_room_event::v3::Request::new(
                room_id,
                event_id,
            ))
            .await
            .ok()?;

        Some(event.event)
    }

    pub async fn get_profile(&self, user_id: String) -> Option<ruma::api::client::profile::get_profile::v3::Response> {

        let parsed_id = ruma::OwnedUserId::try_from(user_id.clone()).ok()?;

        let profile = self.client
            .send_request(get_profile::v3::Request::new(
                parsed_id,
            ))
            .await
            .ok()?;

        Some(profile)
    }


    pub async fn send_message(
        &self, 
        event_type: MessageLikeEventType,
        room_id: OwnedRoomId, 
        message: ruma::serde::Raw<AnyMessageLikeEventContent>
    ) 
    -> Result<String, anyhow::Error> {

        let txn_id = TransactionId::new();

        let req = send_message_event::v3::Request::new_raw(
            room_id,
            txn_id,
            event_type,
            message,
        );

        let response = self.client
            .send_request(req)
            .await?;

        Ok(response.event_id.to_string())
    }

    pub async fn send_to_inbox(
        &self, 
        room_id: OwnedRoomId, 
        subject: String,
        body: String,
        relation: Option<RelatesTo>,
        event_type: Option<String>,
    ) 
    -> Result<String, anyhow::Error> {

        let mut ev_type = MessageLikeEventType::from("matrixbird.email.matrix");

        match event_type {
            Some(et) => {
                ev_type = MessageLikeEventType::from(et);
            },
            None => (),
        }

        let mid_localpart = Uuid::new_v4().to_string();
        let message_id = format!("{}@{}", mid_localpart, "matrixbird.com");


        let from = Address{
            name: Some(String::from("Matrixbird")),
            address: String::from("matrixbird@matrixbird.com")
        };

        let date = Utc::now();

        let body = EmailBody{
            text: None,
            html: Some(body),
        };

        let mut em_cont = EmailContent{
            message_id,
            body,
            from,
            subject: Some(subject),
            date,
            attachments: None,
            m_relates_to: None,
        };

        match relation {
            Some(rel) => {
                em_cont.m_relates_to = Some(rel);
            },
            None => (),
        }


        let raw_event = ruma::serde::Raw::new(&em_cont)?;

        let raw = raw_event.cast::<AnyMessageLikeEventContent>();


        let txn_id = TransactionId::new();

        let req = send_message_event::v3::Request::new_raw(
            room_id,
            txn_id,
            ev_type,
            raw,
        );

        let response = self.client
            .send_request(req)
            .await?;

        Ok(response.event_id.to_string())
    }

}

