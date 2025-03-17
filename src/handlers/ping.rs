use axum::{
    extract::State,
    response::IntoResponse,
    Json,
};

use std::sync::Arc;

use serde_json::json;

use serde::Deserialize;

use crate::AppState;
use crate::error::AppserviceError;


#[derive(Debug, Clone, Deserialize)]
pub struct PingRequest {
    pub transaction_id: String,
}

pub async fn ping(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<PingRequest>,
) -> Result<impl IntoResponse, AppserviceError> {

    let txn_id = payload.transaction_id.clone();

    if !state.transaction_store.verify_and_remove_transaction(&txn_id).await {
        println!("Transaction ID does not match: {}", txn_id);
    }

    Ok(Json(json!({})))
}
