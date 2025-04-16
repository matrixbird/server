use aws_config::BehaviorVersion;
use aws_sdk_s3::{Client, config::{Credentials, Region}};
use bytes::Bytes;
use aws_sdk_s3::primitives::ByteStream;

use crate::config::Config;

#[derive(Clone)]
pub struct Storage {
    pub client: Client,
    pub bucket: String,
}

impl Storage {
    pub async fn new(config: &Config) -> Self {

        let credentials = Credentials::new(
            &config.storage.access_key_id,
            &config.storage.access_key_secret,
            None, 
            None, 
            "R2",
        );

        let r2_config = aws_config::defaults(BehaviorVersion::latest())
            .region(Region::new("auto")) 
            .endpoint_url(&config.storage.endpoint)
            .credentials_provider(credentials)
            .load()
            .await;

        let client = Client::new(&r2_config);

        Self {
            client,
            bucket: config.storage.bucket.clone(),
        }

    }

    pub async fn upload(
        &self,
        key: &str,
        object: &[u8],
    ) -> Result<(), anyhow::Error> {

        let body = ByteStream::from(Bytes::from(object.to_vec()));

        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(body)
            .send()
            .await?;

        tracing::info!("Uploaded to {}/{}", &self.bucket, key);

        Ok(())
    }

}
