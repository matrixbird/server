use crate::config::Config;
use chrono::Utc;

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
            get_state_events_for_key
        },
        room::get_room_event,
        membership::{
            join_room_by_id, 
            leave_room
        },
        profile::get_profile,
    },
    events::{
        AnyMessageLikeEventContent, 
        MessageLikeEventType,
        AnyTimelineEvent,
        AnyStateEvent, 
        StateEventType,
    }
};

use uuid::Uuid;

use anyhow;

use crate::hook::{EmailBody, EmailContent, Address};

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

    pub async fn join_room(&self, room_id: OwnedRoomId) {

        let jr = self.client
            .send_request(join_room_by_id::v3::Request::new(
                room_id
            ))
            .await
            .ok();

        println!("Join room: {:#?}", jr);
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
    -> Result<OwnedEventId, anyhow::Error> {

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

        Ok(response.event_id)
    }

    pub async fn send_welcome_message(
        &self, 
        room_id: OwnedRoomId, 
        subject: String,
        body: String,
    ) 
    -> Result<OwnedEventId, anyhow::Error> {

        let ev_type = MessageLikeEventType::from("matrixbird.email.native");

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

        let em_cont = EmailContent{
            message_id,
            body,
            from,
            subject: Some(subject),
            date,
            attachments: None,
        };

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

        Ok(response.event_id)
    }

}

