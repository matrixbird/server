use crate::config::Config;

use crate::utils::generate_magic_code;

use redis::{
    AsyncCommands,
    RedisError
};

use crate::rooms::PublicRoom;

pub struct Cache {
    pub client: redis::Client,
}

impl Cache {
    pub async fn new(config: &Config) -> Result<Self, anyhow::Error> {
        let url = format!("redis://{}", config.redis.url);
        let client = redis::Client::open(url)?;
        Ok(Self { client })
    }
}

pub async fn get_cached_rooms(
    conn: &mut redis::aio::MultiplexedConnection,
) -> Result<Vec<PublicRoom>, RedisError> {
    let data: String = conn.get("public_rooms").await?;
    serde_json::from_str(&data).map_err(|e| {
        RedisError::from((
            redis::ErrorKind::IoError,
            "Deserialization error",
            e.to_string(),
        ))
    })
}

pub async fn cache_rooms(
    conn: &mut redis::aio::MultiplexedConnection,
    rooms: &Vec<PublicRoom>,
    ttl: u64,
) -> Result<(), RedisError> {
    let serialized = serde_json::to_string(rooms).map_err(|e| {
        RedisError::from((
            redis::ErrorKind::IoError,
            "Serialization error",
            e.to_string(),
        ))
    })?;

    conn.set_ex("public_rooms", serialized, ttl).await
}

pub async fn get_cached_room_state(
    conn: &mut redis::aio::MultiplexedConnection,
    room_id: &str,
) -> Result<Vec<PublicRoom>, RedisError> {

    let key = format!("room_state:{}", room_id);

    let data: String = conn.get(key).await?;
    serde_json::from_str(&data).map_err(|e| {
        RedisError::from((
            redis::ErrorKind::IoError,
            "Deserialization error",
            e.to_string(),
        ))
    })
}

pub async fn store_verification_code(
    conn: &mut redis::aio::MultiplexedConnection,
    email: String,
) -> Result<(), RedisError> {

    let code = generate_magic_code();

    conn.set_ex(email, code, 1500).await
}

pub async fn get_verification_code(
    conn: &mut redis::aio::MultiplexedConnection,
    email: &str,
) -> Result<String, RedisError> {

    let data: String = conn.get(email).await?;

    Ok(data)
}

