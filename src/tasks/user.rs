use std::sync::Arc;
use crate::AppState;

use tokio::time::{sleep, Duration};

use js_int::UInt;

use crate::appservice::HttpClient;

use crate::utils::is_local_room;

use ruma::{
    OwnedRoomId,
    OwnedUserId,
    api::Direction,
    api::client::{
        profile::set_display_name,
        membership::{join_room_by_id, joined_rooms},
        state::get_state_events,
        message::get_message_events,
        threads::get_threads,
        filter::{RoomFilter, RoomEventFilter, FilterDefinition},
        sync::sync_events::v3::Filter,
        sync::sync_events,
    }
};

pub async fn join_room(
    state: Arc<AppState>,
    user_id: OwnedUserId,
    room_id: OwnedRoomId,
) -> Result<(), anyhow::Error> {

    sleep(Duration::from_secs(5)).await;

    let access_token = state.db.access_tokens.get(&user_id.to_string())
        .await?;

    let client = ruma::Client::builder()
        .homeserver_url(state.config.matrix.homeserver.clone())
        .access_token(access_token)
        .build::<HttpClient>()
        .await?;

    let joined_rooms = client
        .send_request(joined_rooms::v3::Request::new())
        .await?;

    if joined_rooms.joined_rooms.contains(&room_id) {
        tracing::info!("Already joined room {}", room_id);
        return Ok(());
    }


    let req = join_room_by_id::v3::Request::new(
        room_id.clone(),
    );

    client.send_request(req).await?;

    tracing::info!("Joined room {}", room_id);


    let is_local = is_local_room(&room_id, &state.config.matrix.server_name);

    if is_local {
        return Ok(());
    }


    tracing::info!("Room is not local, we should fetch state and messages to initiate sync...");

    let _ = client
        .send_request(get_state_events::v3::Request::new(
            room_id.clone(),
        ))
        .await?;

    let mut req = get_message_events::v3::Request::new(
        room_id.clone(),
        Direction::Backward,
    );

    let mut filter = RoomEventFilter::empty();
    filter.unread_thread_notifications = true;
    filter.types = Some(vec!["matrixbird.email.matrix".to_string(), "matrixbird.thread.marker".to_string()]);

    req.filter = filter;

    if let Some(limit) = UInt::new(10) {
        req.limit = limit;
    }

    let messages = client
        .send_request(req)
        .await?;

    println!("Messages length: {:?}", messages.chunk.len());


    let threads = client
        .send_request(get_threads::v1::Request::new(
            room_id.clone(),
        ))
        .await?;

    println!("Threads: {:?}", threads);

    Ok(())
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


