use std::sync::Arc;
use crate::AppState;

use js_int::UInt;

use crate::appservice::HttpClient;

use crate::utils::is_local_room;

use ruma::{
    OwnedRoomId,
    OwnedUserId,
    api::Direction,
    api::client::{
        profile::set_display_name,
        state::get_state_events,
        message::get_message_events,
        filter::RoomEventFilter,
    }
};

pub async fn sync_joined_room(
    state: Arc<AppState>,
    user_id: OwnedUserId,
    room_id: OwnedRoomId,
) -> Result<(), anyhow::Error> {

    let access_token = state.db.access_tokens.get(&user_id.to_string())
        .await?;

    let client = ruma::Client::builder()
        .homeserver_url(state.config.matrix.homeserver.clone())
        .access_token(access_token)
        .build::<HttpClient>()
        .await?;

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

    tracing::info!("Fetched messages: {:?}", messages.chunk.len());

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


