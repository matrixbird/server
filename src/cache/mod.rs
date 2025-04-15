use crate::config::Config;

use redis::AsyncCommands;

use crate::domain::WellKnown;

#[derive(Clone)]
pub struct Cache {
    pub client: redis::Client,
    pub ttl: Option<u64>,
}

impl Cache {
    pub async fn new(config: &Config) -> Result<Self, anyhow::Error> {

        let url = format!("redis://{}", config.redis.cache.url);
        let client = redis::Client::open(url)?;
        let ttl = config.redis.session.ttl;
        Ok(Self { client, ttl })
    }

    pub async fn cache_well_known(
        &self, 
        well_known_url: &str, 
        well_known: &WellKnown,
    ) -> Result<(), anyhow::Error> {

        let mut conn = self.client.get_multiplexed_async_connection().await?;

        let serialized = serde_json::to_string(&well_known)?;

        let ttl = self.ttl.unwrap_or(3600);

        let () = conn.set_ex(
            well_known_url,
            serialized,
            ttl,
        ).await?;
        
        Ok(())
    }

    pub async fn get_well_known(&self, well_known_url: &str) -> Result<Option<WellKnown>, anyhow::Error> {

        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let data = conn.get::<_, Option<String>>(&well_known_url).await?;

        match data {
            Some(data) => {
                let well_known: WellKnown = serde_json::from_str(&data)?;

                Ok(Some(well_known))
            },
            None => {
                Ok(None)
            }
        }

    }


}

