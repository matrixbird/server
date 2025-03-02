use crate::config::Config;

use serde::{Deserialize, Serialize};

use uuid::Uuid;

use ruma::OwnedDeviceId;

use redis::AsyncCommands;

use crate::utils::generate_magic_code;

#[derive(Clone)]
pub struct SessionStore {
    pub client: redis::Client,
    pub ttl: Option<u64>,
}

impl SessionStore {
    pub async fn new(config: &Config) -> Result<Self, anyhow::Error> {

        let url = format!("redis://{}", config.redis.session.url);
        let client = redis::Client::open(url)?;
        let ttl = config.redis.session.ttl;
        Ok(Self { client, ttl })
    }

    pub async fn create_session(&self, user_id: String, access_token: String, device_id: Option<OwnedDeviceId>) -> Result<String, anyhow::Error> {

        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let session_id = Uuid::new_v4().to_string();
        let session = Session::new(user_id, access_token, device_id);
        
        let serialized = serde_json::to_string(&session)?;

        match self.ttl {
            Some(ttl) => {
                let () = conn.set_ex(
                    session_id.clone(),
                    serialized,
                    ttl,
                ).await?;
            },
            None => {
                let () = conn.set(
                    session_id.clone(),
                    serialized,
                ).await?;
            }
        }
        
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


    pub async fn validate_session(&self, session_id: &str) -> Result<(bool, Option<Session>), anyhow::Error> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        // Get session and update last_access
        if let Some(data) = conn.get::<_, Option<String>>(&session_id).await? {
            let mut session: Session = serde_json::from_str(&data)?;


            session.last_access = chrono::Utc::now().timestamp();

            let serialized = serde_json::to_string(&session)?;
            
            match self.ttl {
                Some(ttl) => {
                    let () = conn.set_ex(
                        session_id,
                        serialized,
                        ttl,
                    ).await?;
                },
                None => {
                    let () = conn.set(
                        session_id,
                        serialized,
                    ).await?;
                }
            }

            
            Ok((true, Some(session)))
        } else {
            Ok((false, None))
        }
    }

    pub async fn revoke_session(&self, session_id: &str) -> Result<(), anyhow::Error> {
        tracing::info!("Revoking session: {}", session_id);
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let () = conn.del(session_id).await?;

        Ok(())
    }

    pub async fn create_verification_code(&self, email: String, client_secret:String ) -> Result<(String, String), anyhow::Error> {

        let code = generate_magic_code();
        println!("Verification code: {}", code);
        let session = Uuid::new_v4().to_string();

        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let req = VerificationRequest {
            email,
            client_secret,
            code: code.clone(),
        };

        let serialized = serde_json::to_string(&req)?;
        
        // Store session with TTL
        let () = conn.set_ex(
            session.clone(),
            serialized,
            1800,
        ).await?;
        
        Ok((session, code))
    }

    pub async fn verify_code(&self, session: String, email: String, client_secret: String, code: String ) -> Result<bool, anyhow::Error> {

        let mut conn = self.client.get_multiplexed_async_connection().await?;
        if let Some(data) = conn.get::<_, Option<String>>(&session).await? {
            let request: VerificationRequest = serde_json::from_str(&data)?;
            println!("Request: {:#?}", request);

            if request.code == code && 
                request.client_secret == client_secret &&
                request.email == email {
                return Ok(true)
            }

            return Ok(true)
        }

        Ok(false)
    }

    pub async fn get_code_session(&self, session: String) -> Result<Option<VerificationRequest>, anyhow::Error> {

        let mut conn = self.client.get_multiplexed_async_connection().await?;
        if let Some(data) = conn.get::<_, Option<String>>(&session).await? {
            let request: VerificationRequest = serde_json::from_str(&data)?;
            return Ok(Some(request))
        }

        Ok(None)
    }

}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationRequest {
    pub email: String,
    pub client_secret: String,
    pub code: String,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub user_id: String,
    pub access_token: String,
    pub device_id: Option<OwnedDeviceId>,
    pub created_at: i64,
    pub last_access: i64,
}

impl Session {
    fn new(user_id: String, access_token: String, device_id: Option<OwnedDeviceId>) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            user_id,
            access_token,
            device_id,
            created_at: now,
            last_access: now,
        }
    }
}

