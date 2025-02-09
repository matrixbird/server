use axum::{
    extract::{Path, State, Query},
    http::StatusCode,
    response::IntoResponse,
    Json,
    Extension
};

use ruma::{
    RoomId,
    EventId,
    MilliSecondsSinceUnixEpoch,
    OwnedMxcUri,
    events::{
        AnyTimelineEvent,
        room::{
            create::{RoomCreateEvent, RoomCreateEventContent},
            name::RoomNameEventContent,
            canonical_alias::RoomCanonicalAliasEventContent,
            avatar::RoomAvatarEventContent,
            topic::RoomTopicEventContent,
            history_visibility::RoomHistoryVisibilityEventContent,
            join_rules::{RoomJoinRulesEventContent, JoinRule},
        },
        space::child::SpaceChildEventContent, 
    }
};

use serde::{Serialize, Deserialize};
use serde_json::{
    json, 
    Value
};

use std::sync::Arc;

use crate::AppState;
use crate::appservice::{
    JoinedRoomState,
    RoomSummary
};

use crate::middleware::Data;

use crate::utils;

use crate::cache::{
    get_cached_rooms,
    cache_rooms
};

use crate::error::AppserviceError;

use tracing::{info, warn};

pub async fn public_rooms (
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppserviceError> {

    // read from cache if enabled
    if state.config.cache.public_rooms.enabled {
        let mut redis_conn = state.cache.get_multiplexed_async_connection()
            .await;

        if let Ok(ref mut redis_conn) = redis_conn {
            if let Ok(cached_data) = get_cached_rooms(redis_conn).await {
                info!("Public rooms fetched from cache");
                return Ok((
                    StatusCode::OK,
                    Json(json!({
                        "rooms": json!(cached_data),
                    }))
                ))
            }
        }
    }

    let rooms = state.appservice.joined_rooms_state()
        .await 
        .map_err(|_| AppserviceError::MatrixError("Failed to fetch rooms".to_string()))?;


    let processed = process_rooms(rooms);

    let to_cache = processed.clone();

    // cache public rooms if enabled
    if state.config.cache.public_rooms.enabled {
        tokio::spawn(async move {
            let mut redis_conn = state.cache.get_multiplexed_async_connection()
                .await;

            if let Ok(ref mut redis_conn) = redis_conn {
                if let Err(e) = cache_rooms(
                    redis_conn, 
                    &to_cache,
                    state.config.cache.public_rooms.expire_after
                ).await {
                    warn!("Failed to cache public rooms: {}", e);
                } else {
                    info!("Public rooms cached");
                }
            }
        });
    }

    Ok((
        StatusCode::OK,
        Json(json!({
            "rooms": json!(processed),
        }))
    ))
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct PublicRoom {
    room_id: String,
    #[serde(rename = "type")]
    #[serde(skip_serializing_if = "Option::is_none")]
    room_type: Option<String>,
    origin_server_ts: Option<MilliSecondsSinceUnixEpoch>,
    #[serde(rename = "room_type")]
    #[serde(skip_serializing_if = "Option::is_none")]
    commune_room_type: Option<String>,
    name: Option<String>,
    canonical_alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    commune_alias: Option<String>,
    sender: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    avatar_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    banner_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    topic: Option<String>,
    join_rule: Option<String>,
    history_visibility: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    children: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    settings: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    room_categories: Option<Vec<Value>>,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_false")]
    is_bridge: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn process_rooms(rooms: Vec<JoinedRoomState>) -> Vec<PublicRoom> {

    let mut public_rooms: Vec<PublicRoom> = Vec::new();

    for room in &rooms {

        let mut pub_room = PublicRoom {
            room_id: room.room_id.to_string(),
            ..Default::default()
        };

        for state_event in &room.state.clone().unwrap_or_else(|| Vec::new()) {

            let event_type = match state_event.get_field::<String>("type") {
                Ok(Some(t)) => t,
                Ok(None) => {
                    continue;
                }
                Err(_) => {
                    continue;
                }
            };

            if event_type == "m.room.create" {
                match state_event.deserialize_as::<RoomCreateEvent>() {
                    Ok(event) => {
                        pub_room.origin_server_ts = Some(event.origin_server_ts());
                        pub_room.sender = Some(event.sender().to_string());

                    }
                    Err(_) => () 
                }

                if let Ok(Some(content)) = state_event.get_field::<RoomCreateEventContent>("content") {
                    if let Some(room_type) = content.room_type {
                        pub_room.room_type = Some(room_type.to_string());
                    }
                };
            }

            // don't overwrite the name if commune.room.name is already set
            if event_type == "m.room.name" && pub_room.name.is_none() {
                if let Ok(Some(content)) = state_event.get_field::<RoomNameEventContent>("content") {
                    pub_room.name = Some(content.name.to_string());
                };
            }

            if event_type == "commune.room.name" {
                if let Ok(Some(content)) = state_event.get_field::<RoomNameEventContent>("content") {

                    pub_room.name = Some(content.name.to_string());
                };
            }

            if event_type == "m.room.canonical_alias" {
                if let Ok(Some(content)) = state_event.get_field::<RoomCanonicalAliasEventContent>("content") {
                    pub_room.canonical_alias = content.alias.map(|a| a.to_string());
                };
            }

            if event_type == "m.room.avatar" {
                if let Ok(Some(content)) = state_event.get_field::<RoomAvatarEventContent>("content") {
                    pub_room.avatar_url = content.url.map(|u| u.to_string());
                };
            }

            if event_type == "m.room.topic" {
                if let Ok(Some(content)) = state_event.get_field::<RoomTopicEventContent>("content") {
                    pub_room.topic = Some(content.topic.to_string());
                };
            }

            if event_type == "m.room.history_visibility" {
                if let Ok(Some(content)) = state_event.get_field::<RoomHistoryVisibilityEventContent>("content") {
                    pub_room.history_visibility = content.history_visibility.to_string();
                };
            }

            if event_type == "commune.room.banner" {
                if let Ok(Some(content)) = state_event.get_field::<RoomAvatarEventContent>("content") {
                    pub_room.banner_url = content.url.map(|u| u.to_string());
                };
            }

            if event_type == "commune.room.type" {
                if let Ok(Some(content)) = state_event.get_field::<CommuneRoomType>("content") {
                    pub_room.room_type = content.room_type.map(|t| t.to_string());
                };
            }

            if event_type == "m.room.join_rules" {
                if let Ok(Some(content)) = state_event.get_field::<RoomJoinRulesEventContent>("content") {
                    pub_room.join_rule = Some(content.join_rule.as_str().to_string());
                };
            }

            let bridge_types = vec!["m.bridge", "m.room.bridged", "m.room.discord", "m.room.irc", "uk.half-shot.bridge"];

            if bridge_types.contains(&event_type.as_str()) {
                pub_room.is_bridge = true;
            }

            if event_type == "m.space.child" {


                if let Ok(Some(content)) = state_event.get_field::<SpaceChildEventContent>("content") {
                    if content.via.len() == 0 {
                        continue;
                    }
                };

                if let Ok(Some(state_key)) = state_event.get_field::<String>("state_key") {
                    let mut _is_public = false;

                    // find the room in the rooms vec
                    if let Some(child_room) = rooms.iter().find(|r| r.room_id == state_key) {


                        for state_event in &child_room.state.clone().unwrap_or_else(|| Vec::new()) {

                            let event_type = match state_event.get_field::<String>("type") {
                                Ok(Some(t)) => t,
                                Ok(None) => {
                                    continue;
                                }
                                Err(_) => {
                                    continue;
                                }
                            };

                            if event_type == "m.room.join_rules" {
                                if let Ok(Some(content)) = state_event.get_field::<RoomJoinRulesEventContent>("content") {

                                    if matches!(content.join_rule, JoinRule::Public) {
                                        //is_public = true;
                                        break; 
                                    }
                                };
                            }

                        }

                        
                    }




                    match pub_room.children {
                        Some(ref mut children) => {
                            children.push(state_key);
                        }
                        None => {
                            pub_room.children = Some(vec![state_key]);
                        }
                    }

                };

            }

        }

        if let Some(name) =  &pub_room.name {
            if name.contains("[⛓️]") {
                continue
            } 
        } 
        public_rooms.push(pub_room);
    }

    public_rooms
}

#[derive(Debug, Deserialize, Serialize)]
struct CommuneRoomType {
    #[serde(rename = "type")]
    room_type: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RoomInfoParams {
    pub room: Option<String>,
    pub event: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RoomInfo {
    #[serde(skip_deserializing)]
    info: RoomSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    room: Option<RoomSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    event: Option<ruma::serde::Raw<AnyTimelineEvent>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sender: Option<Sender>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Sender {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<OwnedMxcUri>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub displayname: Option<String>,
}

pub async fn room_info (
    Path(params): Path<Vec<(String, String)>>,
    Extension(data): Extension<Data>,
    Query(query): Query<RoomInfoParams>,
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppserviceError> {

    let mut room_id = params[0].1.clone();

    if let Some(id) = data.room_id.as_ref() {
        room_id = id.to_string();
    }

    println!("query: {:#?}", query);

    let mut parsed_id = RoomId::parse(&room_id)
        .map_err(|_| AppserviceError::MatrixError("Invalid room ID".to_string()))?;

    let summary =  state.appservice.get_room_summary(parsed_id.clone())
        .await.ok_or(AppserviceError::MatrixError("Room not found".to_string()))?;
    let mut info = RoomInfo {
        info: summary,
        room: None,
        event: None,
        sender: None,
    };

    if let Some(alias) = query.room {

        let hierarchy = state.appservice.get_room_hierarchy(parsed_id.clone()).await;

        if let Some(hierarchy) = hierarchy {
            for room in hierarchy {

                if let Some(name) = room.name.as_ref() {
                    let slug = utils::slugify(name);

                    if slug == alias {
                        parsed_id = room.room_id.clone();

                        let summary =  state.appservice.get_room_summary(parsed_id.clone())
                            .await.ok_or(AppserviceError::MatrixError("Room not found".to_string()))?;

                        info.room = Some(summary);
                        break;
                    }

                }

            }
        }

    }

    if let Some(event_id) = query.event {
        let parsed_event_id = EventId::parse(&event_id)
            .map_err(|_| AppserviceError::MatrixError("Invalid event ID".to_string()))?;

        if let Some(event) = state.appservice.get_room_event(parsed_id, parsed_event_id).await {

            info.event = Some(event.clone());

            if let Ok(Some(sender)) = event.get_field::<String>("sender") {
                println!("sender: {:#?}", sender);

                let profile = state.appservice.get_profile(sender).await;

                if let Some(profile) = profile {
                    info.sender = Some(Sender {
                        avatar_url: profile.avatar_url,
                        displayname: profile.displayname,
                    });
                }

            }

        }

    }


    Ok((
        StatusCode::OK,
        Json(json!(info))
    ))
}
