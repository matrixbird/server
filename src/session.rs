use crate::config::Config;

use crate::utils::generate_magic_code;

use serde::{Deserialize, Serialize};

use uuid::Uuid;



use redis::{
    AsyncCommands,
    RedisError
};

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

    pub async fn create_session(&self, user_id: String, access_token: String) -> Result<String, anyhow::Error> {

        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let session_id = Uuid::new_v4().to_string();
        let session = Session::new(user_id, access_token);
        
        let serialized = serde_json::to_string(&session)?;
        // Store session with TTL
        let () = conn.set_ex(
            session_id.clone(),
            serialized,
            self.ttl,
        ).await?;
        
        Ok(session_id)
    }


    pub async fn validate_session(&self, session_id: &str) -> Result<Option<Session>, anyhow::Error> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        // Get session and update last_access
        if let Some(data) = conn.get::<_, Option<String>>(&session_id).await? {
            let mut session: Session = serde_json::from_str(&data)?;
            session.last_access = chrono::Utc::now().timestamp();

            let serialized = serde_json::to_string(&session)?;
            
            // Reset TTL and update last_access
            let () = conn.set_ex(
                &key,
                serialized,
                self.ttl,
            ).await?;
            
            Ok(Some(session))
        } else {
            Ok(None)
        }
    }

    pub async fn revoke_session(&self, session_id: &str) -> Result<(), anyhow::Error> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let () = conn.del(session_id).await?;

        Ok(())
    }

}

#[derive(Serialize, Deserialize)]
pub struct Session {
    user_id: String,
    matrix_token: String,
    created_at: i64,
    last_access: i64,
}

impl Session {
    fn new(user_id: String, matrix_token: String) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            user_id,
            matrix_token,
            created_at: now,
            last_access: now,
        }
    }
}

