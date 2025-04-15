use crate::config::Config;
use crate::appservice::HttpClient;

use serde_json::json;
use serde::{Serialize, Deserialize};

use std::time::Duration;

use ruma::
    api::client::{
        session::login,
        uiaa::UserIdentifier,
    };

use crate::utils::construct_matrix_id;

#[derive(Clone, Debug)]
pub struct Admin {
    pub base_url: String,
    pub access_token: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct VerifyAdmin {
    pub admin: bool,
}

impl Admin {
    pub async fn new(config: &Config) -> Self {

        let client = ruma::Client::builder()
            .homeserver_url(config.matrix.homeserver.clone())
            .build::<HttpClient>()
            .await
            .unwrap();


        let user_id = match construct_matrix_id(&config.admin.user, &config.matrix.server_name) {
            Some(id) => id,
            None => {
                tracing::error!("Couldn't construct user id");
                std::process::exit(1);
            }
        };

        let id = UserIdentifier::UserIdOrLocalpart(user_id.clone());

        let pass = login::v3::Password::new(
            id,
            config.admin.password.clone()
        );

        let info = login::v3::LoginInfo::Password(pass);


        let resp = client
            .send_request(login::v3::Request::new(
                info
            ))
            .await
            .unwrap_or_else(|e| {
                tracing::error!("Failed to login as admin user: {}", e);
                println!("Make sure the admin user {} exists and the password is correct.", user_id);
                std::process::exit(1);
            });


        let base_url = format!("{}/_synapse/admin/v1", config.matrix.homeserver);

        let is_admin = Self::verify_admin(
                &base_url,
                &resp.access_token,
                &user_id
            ).await.unwrap_or_else(|e| {
                tracing::error!("Failed to verify admin user. {}", e);
                println!("Make sure the user {} is an admin.", user_id);
                std::process::exit(1);
            });

        if !is_admin {
            tracing::error!("User {} is not an admin.", user_id);
            println!("Make sure the user {} is an admin.", user_id);
            std::process::exit(1);
        }

        Self { 
            base_url,
            access_token: resp.access_token,
        }
    }

    pub async fn verify_admin(
        base_url: &str,
        access_token: &str,
        user_id: &str,
    ) -> Result<bool, anyhow::Error> {

        let url = format!("{}/users/{}/admin", base_url, user_id);

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .connect_timeout(Duration::from_secs(3)) 
            .build()?;

        let response = client
            .get(&url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to verify that user is admin: {}", e))?;

        let resp = response.json::<VerifyAdmin>().await
            .map_err(|e| anyhow::anyhow!("Failed to parse admin response. {}", e))?;

        Ok(resp.admin)
    }

    pub async fn reset_password(
        &self,
        user_id: &str,
        new_password: &str,
    ) -> Result<(), anyhow::Error> {

        let url = format!("{}/reset_password/{}", self.base_url, user_id);

        let body = json!({
            "new_password": new_password,
            "logout_devices": true,
        });

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .connect_timeout(Duration::from_secs(3)) 
            .build()?;

        client
            .post(&url)
            .bearer_auth(&self.access_token)
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to reset password: {}", e))?;

        Ok(())
    }

}

