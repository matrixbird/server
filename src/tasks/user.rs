use std::sync::Arc;
use crate::AppState;

use crate::appservice::HttpClient;

use ruma::{
    OwnedUserId,
    api::client::profile::set_display_name,
};

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


