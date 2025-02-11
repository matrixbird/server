use crate::config::Config;

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
        space::{get_hierarchy, SpaceHierarchyRoomsChunk}
    },
    events::{
        AnyMessageLikeEventContent, 
        MessageLikeEventType,
        AnyTimelineEvent,
        AnyStateEvent, 
        StateEventType,
        room::{
            name::RoomNameEventContent,
            canonical_alias::RoomCanonicalAliasEventContent,
            avatar::RoomAvatarEventContent,
            topic::RoomTopicEventContent,
        }
    }
};

use anyhow;

use serde::{Serialize, Deserialize};

pub type HttpClient = ruma::client::http_client::HyperNativeTls;

#[derive(Clone)]
pub struct AppService {
    client: ruma::Client<HttpClient>,
    pub appservice_id: String,
    pub user_id: Box<OwnedUserId>,
}

pub type RoomState = Vec<ruma::serde::Raw<AnyStateEvent>>;

#[derive(Clone)]
pub struct JoinedRoomState {
    pub room_id: OwnedRoomId,
    pub state: Option<RoomState>,
}

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
            eprintln!("Failed to authenticate with homeserver. Check your access token.");
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

    pub async fn joined_rooms_state(&self) -> Result<Vec<JoinedRoomState>, anyhow::Error> {

        let mut joined_rooms: Vec<JoinedRoomState> = Vec::new();

        let jr = self.client
            .send_request(joined_rooms::v3::Request::new())
            .await
            .map_err(|e| anyhow::anyhow!("Error getting joined rooms: {}", e))?;

        if jr.joined_rooms.len() == 0 {
            return Ok(joined_rooms);
        }

        for room_id in jr.joined_rooms {

            let mut jrs = JoinedRoomState {
                room_id: room_id.clone(),
                state: None,
            };


            let st = self.client
                .send_request(get_state_events::v3::Request::new(
                    room_id,
                ))
                .await?;

            jrs.state = Some(st.room_state);

            joined_rooms.push(jrs);

        }

        Ok(joined_rooms)
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

    pub async fn get_room_summary(&self, room_id: OwnedRoomId) ->
    Option<RoomSummary> {

        let mut room_info = RoomSummary {
            room_id: room_id.to_string(),
            ..Default::default()
        };

        let state = self.client
            .send_request(get_state_events::v3::Request::new(
                room_id,
            ))
            .await
            .ok()?;

        for state_event in state.room_state {

            let event_type = match state_event.get_field::<String>("type") {
                Ok(Some(t)) => t,
                Ok(None) => {
                    continue;
                }
                Err(_) => {
                    continue;
                }
            };

            if event_type == "m.room.name" {
                if let Ok(Some(content)) = state_event.get_field::<RoomNameEventContent>("content") {
                    room_info.name = Some(content.name.to_string());
                };
            }

            if event_type == "m.room.canonical_alias" {
                if let Ok(Some(content)) = state_event.get_field::<RoomCanonicalAliasEventContent>("content") {
                    room_info.canonical_alias = content.alias.map(|a| a.to_string());
                };
            }

            if event_type == "m.room.avatar" {
                if let Ok(Some(content)) = state_event.get_field::<RoomAvatarEventContent>("content") {
                    room_info.avatar_url = content.url.map(|u| u.to_string());
                };
            }

            if event_type == "commune.room.banner" {
                if let Ok(Some(content)) = state_event.get_field::<RoomAvatarEventContent>("content") {
                    room_info.banner_url = content.url.map(|u| u.to_string());
                };
            }

            if event_type == "m.room.topic" {
                if let Ok(Some(content)) = state_event.get_field::<RoomTopicEventContent>("content") {
                    room_info.topic = Some(content.topic.to_string());
                };
            }
        }

        Some(room_info)
    }

    pub async fn get_room_hierarchy(&self, room_id: OwnedRoomId) -> Option<Vec<SpaceHierarchyRoomsChunk>> {

        let hierarchy = self.client
            .send_request(get_hierarchy::v1::Request::new(
                room_id
            ))
            .await
            .ok()?;

        Some(hierarchy.rooms)
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

}

#[derive(Default, Debug, Deserialize, Serialize)]
pub struct RoomSummary {
    pub room_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub banner_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
}

