use crate::config::Config;

use serde::{Deserialize, Serialize};

use uuid::Uuid;

use ruma::OwnedDeviceId;

use redis::AsyncCommands;

#[derive(Clone)]
pub struct SessionStore {
    pub client: redis::Client,
    pub ttl: u64,
}

impl SessionStore {
    pub async fn new(config: &Config) -> Result<Self, anyhow::Error> {

        let url = format!("redis://{}", config.redis.session.url);
        let client = redis::Client::open(url)?;
        let ttl = config.redis.session.ttl;
        Ok(Self { client, ttl })
    }

    pub async fn create_session(&self, user: String, access_token: String, device_id: Option<OwnedDeviceId>) -> Result<String, anyhow::Error> {

        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let session_id = Uuid::new_v4().to_string();
        let session = Session::new(user, access_token, device_id);
        
        let serialized = serde_json::to_string(&session)?;
        // Store session with TTL
        let () = conn.set_ex(
            session_id.clone(),
            serialized,
            self.ttl,
        ).await?;
        
        Ok(session_id)
    }

    pub async fn get_session(&self, session_id: &str) -> Result<Option<Session>, anyhow::Error> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        if let Some(data) = conn.get::<_, Option<String>>(&session_id).await? {
            let session: Session = serde_json::from_str(&data)?;

            return Ok(Some(session))
        }

        Ok(None)
    }


    pub async fn validate_session(&self, session_id: &str, device_id: &str) -> Result<bool, anyhow::Error> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        // Get session and update last_access
        if let Some(data) = conn.get::<_, Option<String>>(&session_id).await? {
            let mut session: Session = serde_json::from_str(&data)?;

            if session.device_id.is_none() {
                return Ok(false);
            }

            if let Some(ref id) = session.device_id {
                if id.as_str() != device_id {
                    return Ok(false);
                }
            }


            session.last_access = chrono::Utc::now().timestamp();

            let serialized = serde_json::to_string(&session)?;
            
            // Reset TTL and update last_access
            let () = conn.set_ex(
                session_id,
                serialized,
                self.ttl,
            ).await?;
            
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub async fn revoke_session(&self, session_id: &str) -> Result<(), anyhow::Error> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let () = conn.del(session_id).await?;

        Ok(())
    }

}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub user: String,
    pub access_token: String,
    pub device_id: Option<OwnedDeviceId>,
    pub created_at: i64,
    pub last_access: i64,
}

impl Session {
    fn new(user: String, access_token: String, device_id: Option<OwnedDeviceId>) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            user,
            access_token,
            device_id,
            created_at: now,
            last_access: now,
        }
    }
}

