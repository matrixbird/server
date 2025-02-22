use crate::config::Config;

//use crate::utils::generate_magic_code;

//use redis::{
    //AsyncCommands,
    //RedisError
//};


pub struct Cache {
    pub client: redis::Client,
}

impl Cache {
    pub async fn new(config: &Config) -> Result<Self, anyhow::Error> {
        let url = format!("redis://{}", config.redis.cache.url);
        let client = redis::Client::open(url)?;
        Ok(Self { client })
    }
}

