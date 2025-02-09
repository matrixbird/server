use axum::{
    body::Body,
    extract::{
        Path, 
        State, 
        OriginalUri, 
        MatchedPath
    },
    http::{
        Request, 
        StatusCode, 
        Uri, 
        header::AUTHORIZATION
    },
    response::IntoResponse,
    middleware::Next,
    Json,
    Extension
};

use ruma::{
    RoomId, 
    RoomAliasId,
};

use serde_json::{
    json, 
    Value
};

use std::sync::Arc;

use crate::AppState;
use crate::utils::room_id_valid;

use crate::error::AppserviceError;

pub async fn authenticate_homeserver(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
) -> Result<impl IntoResponse, (StatusCode, Json<Value>)> {

    if let Some(auth_header) = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok()) {
        if let Some(token) = extract_token(auth_header) {
            if token == &state.config.appservice.hs_access_token {
                return Ok(next.run(req).await)
            }
        }
    };

    Err((
        StatusCode::UNAUTHORIZED,
        Json(json!({
            "errcode": "M_FORBIDDEN",
        }))
    ))
}

pub fn extract_token(header: &str) -> Option<&str> {
    if header.starts_with("Bearer ") {
        Some(header.trim_start_matches("Bearer ").trim())
    } else {
        None
    }
}

#[derive(Clone)]
pub struct Data {
    pub modified_path: Option<String>,
    pub room_id: Option<String>,
}

pub async fn validate_room_id(
    Path(params): Path<Vec<(String, String)>>,
    State(state): State<Arc<AppState>>,
    mut req: Request<Body>,
    next: Next,
) -> Result<impl IntoResponse, (StatusCode, Json<Value>)> {

    let room_id = params[0].1.clone();

    let server_name = state.config.matrix.server_name.clone();

    let mut data = Data {
        modified_path: None,
        room_id: Some(room_id.clone()),
    };

    if let Err(_) = room_id_valid(&room_id, &server_name) {

        let raw_alias = format!("#{}:{}", room_id, server_name);

        if let Ok(alias) = RoomAliasId::parse(&raw_alias) {
            let id = state.appservice.room_id_from_alias(alias).await;
            match id {
                Some(id) => {
                    println!("Fetched Room ID: {:#?}", id);
                    data.room_id = Some(id.to_string());
                }
                None => {
                    println!("Failed to get room ID from alias: {}", raw_alias);
                }
            }
        }


        if let Some(path) = req.extensions().get::<MatchedPath>() {
            let pattern = path.as_str();
            
            // Split into segments, skipping the empty first segment
            let pattern_segments: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();

            let fullpath = if let Some(path) = req.extensions().get::<OriginalUri>() {
                path.0.path()
            } else {
                req.uri().path()
            };

            let path_segments: Vec<&str> = fullpath.split('/').filter(|s| !s.is_empty()).collect();
            
            if let Some(segment_index) = pattern_segments.iter().position(|&s| s == ":room_id") {
                println!("Found :room_id at segment index: {}", segment_index);
                let mut new_segments = path_segments.clone();
                if segment_index < new_segments.len() {

                    new_segments[segment_index] = data.room_id.as_ref().unwrap();
                    
                    // Rebuild the path with leading slash
                    let new_path = format!("/{}", new_segments.join("/"));
                    
                    // Preserve query string if it exists
                    let new_uri = if let Some(query) = req.uri().query() {
                        format!("{}?{}", new_path, query).parse::<Uri>().unwrap()
                    } else {
                        new_path.parse::<Uri>().unwrap()
                    };
                    
                    data.modified_path = Some(new_uri.to_string());
                }
            }
        }

    }

    req.extensions_mut().insert(data);

    Ok(next.run(req).await)
}


pub async fn validate_public_room(
    Extension(data): Extension<Data>,
    //Path(params): Path<Vec<(String, String)>>,
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
) -> Result<impl IntoResponse, AppserviceError> {

    let room_id = data
        .room_id
        .as_ref()
        .ok_or(AppserviceError::AppserviceError("No room ID found".to_string()))?;

    let id = RoomId::parse(room_id)
        .map_err(|_| AppserviceError::AppserviceError("Invalid room ID".to_string()))?;

    if !state.appservice.has_joined_room(id).await {
        return Err(AppserviceError::AppserviceError("User is not in room".to_string()));
    }

    Ok(next.run(req).await)
}
