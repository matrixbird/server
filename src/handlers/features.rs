pub use crate::AppState;

use serde_json::json;
use std::sync::Arc;

use axum::{
    extract::State,
    response::IntoResponse,
    Json,
};

pub async fn features(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ()> {

    let mut features = json!({});

    features["email"]["outgoing"] = state.config.email.outgoing.enabled.into();

    Ok(Json(json!(features)))
}

pub async fn authentication_features(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ()> {

    Ok(Json(json!(state.config.features.authentication)))
}

