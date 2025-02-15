use reqwest::Client;
use serde::{Deserialize, Serialize};
use anyhow::Result;

#[derive(Serialize)]
struct EmailRequest {
    #[serde(rename = "From")]
    from: String,
    #[serde(rename = "To")]
    to: String,
    #[serde(rename = "Subject")]
    subject: Option<String>,
    #[serde(rename = "HtmlBody")]
    html_body: Option<String>,
    #[serde(rename = "MessageStream")]
    message_stream: Option<String>,
    #[serde(rename = "TemplateAlias")]
    template_alias: Option<String>,
    #[serde(rename = "TemplateModel")]
    template_model: Option<TemplateModel>,
}

#[derive(Serialize)]
pub struct TemplateModel {
    code: String,
}

#[derive(Deserialize, Debug)]
pub struct PostmarkResponse {
    #[serde(rename = "To")]
    pub to: String,
    #[serde(rename = "SubmittedAt")]
    pub submitted_at: String,
    #[serde(rename = "MessageID")]
    pub message_id: String,
    #[serde(rename = "ErrorCode")]
    pub error_code: i32,
    #[serde(rename = "Message")]
    pub message: String,
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

    pub async fn send_email_template(
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
            template_alias: Some("verification-code".to_string()),
            template_model: Some(template_model),
            html_body: None,
            subject: None,
            message_stream: None,
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

    pub async fn send_email(
        &self,
        to: &str,
        template: &str,
        subject: &str,
    ) -> Result<PostmarkResponse> {
        let client = Client::new();

        let email = EmailRequest {
            from: self.account.to_string(),
            to: to.to_string(),
            template_alias: None,
            template_model: None,
            html_body: Some(template.to_string()),
            subject: Some(subject.to_string()),
            message_stream: Some("outbound".to_string()),
        };

        let response = client
            .post("https://api.postmarkapp.com/email")
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .header("X-Postmark-Server-Token", &self.api_token)
            .json(&email)
            .send()
            .await?;

        println!("Response: {:#?}", response);

        let result = response.json::<PostmarkResponse>().await?;
        println!("Response: {:#?}", result);
        
        Ok(result)
    }
}

