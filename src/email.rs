extern crate mailchecker;

use std::collections::HashSet;
use std::fs;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct EmailProviders {
    pub providers: HashSet<String>,
}

impl EmailProviders {

    pub fn new(path: &str) -> Result<Self, anyhow::Error> {
        let contents = fs::read_to_string(path)?;
        let providers: Vec<String> = serde_json::from_str(&contents)?;
        Ok(Self {
            providers: providers.into_iter().collect(),
        })
    }
    
    /// Check if a provider exists
    pub fn contains(&self, provider: &str) -> bool {
        self.providers.contains(provider)
    }
    
    /// Get the number of providers
    pub fn len(&self) -> usize {
        self.providers.len()
    }
    
    /// Check if the providers list is empty
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
    
    /// Extract the provider from an email address
    pub fn extract_provider(email: &str) -> Option<&str> {
        email.split('@').nth(1)
    }
    
    /// Check if an email address uses a known provider
    pub async fn reject(&self, email: &str) -> bool {
        let not_disposable = mailchecker::is_valid(email);
        if !not_disposable {
            println!("Email is disposable");
            return true;
        }

        if let Some(provider) = Self::extract_provider(email) {
            self.contains(provider)
        } else {
            false
        }
    }
}


#[derive(Serialize)]
struct EmailRequest {
    #[serde(rename = "From")]
    from: String,
    #[serde(rename = "To")]
    to: String,
    #[serde(rename = "TemplateAlias")]
    template_alias: String,
    #[serde(rename = "TemplateModel")]
    template_model: TemplateModel,
}

#[derive(Serialize)]
pub struct TemplateModel {
    code: String,
}

#[derive(Deserialize, Debug)]
pub struct PostmarkResponse {
    #[serde(rename = "To")]
    to: String,
    #[serde(rename = "SubmittedAt")]
    submitted_at: String,
    #[serde(rename = "MessageID")]
    message_id: String,
    #[serde(rename = "ErrorCode")]
    error_code: i32,
    #[serde(rename = "Message")]
    message: String,
}

#[derive(Debug, Clone)]
pub struct EmailClient {
    api_token: String,
    account: String,
}

impl EmailClient {
    pub fn new(api_token: &str, account: &str) -> Self {
        Self {
            api_token: api_token.to_string(),
            account: account.to_string(),
        }
    }

    pub async fn send_email(
        &self,
        to: &str,
        code: &str,
    ) -> Result<PostmarkResponse> {
        let client = Client::new();

        let template_model = TemplateModel {
            code: code.to_string(),
        };
        
        let email = EmailRequest {
            from: self.account.to_string(),
            to: to.to_string(),
            template_alias: "verification-code".to_string(),
            template_model,
        };

        let response = client
            .post("https://api.postmarkapp.com/email/withTemplate")
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .header("X-Postmark-Server-Token", &self.api_token)
            .json(&email)
            .send()
            .await?;

        let result = response.json::<PostmarkResponse>().await?;
        println!("Response: {:#?}", result);
        
        Ok(result)
    }
}

pub async fn send_email(
    api_token: &str,
    from: &str,
    to: &str,
    code: &str,
) -> Result<PostmarkResponse> {
    let client = Client::new();

    let template_model = TemplateModel {
        code: code.to_string(),
    };
    
    let email = EmailRequest {
        from: from.to_string(),
        to: to.to_string(),
        template_alias: "verification-code".to_string(),
        template_model,
    };

    let response = client
        .post("https://api.postmarkapp.com/email/withTemplate")
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .header("X-Postmark-Server-Token", api_token)
        .json(&email)
        .send()
        .await?;

    let result = response.json::<PostmarkResponse>().await?;
    println!("Response: {:#?}", result);
    
    Ok(result)
}

