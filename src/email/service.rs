use crate::config::{Config, SMTP, EmailDomains};

use lettre::{
    message::header::{Header, HeaderName, HeaderValue},
    transport::smtp::authentication::{Credentials, Mechanism},
    transport::smtp::client::Tls,
    Message, SmtpTransport, Transport,
};

use lettre::message::{MultiPart, SinglePart};

use lettre::message::header::ContentType;

use std::time::Duration;
use std::error::Error;

use serde_json::Value;


#[derive(Debug, Clone)]
struct XPMMessageStream(String);

impl Header for XPMMessageStream {
    fn name() -> HeaderName {
        HeaderName::new_from_ascii_str("X-PM-Message-Stream")
    }

    fn parse(s: &str) -> Result<Self, Box<dyn Error + Send + Sync>> {
        Ok(XPMMessageStream(s.to_string()))
    }

    fn display(&self) -> HeaderValue {
        let name = HeaderName::new_from_ascii_str("X-PM-Message-Stream");
        HeaderValue::new(name, self.0.clone())
    }
}

#[derive(Debug, Clone)]
struct InReplyTo(String);

impl Header for InReplyTo {
    fn name() -> HeaderName {
        HeaderName::new_from_ascii_str("In-Reply-To")
    }

    fn parse(s: &str) -> Result<Self, Box<dyn Error + Send + Sync>> {
        Ok(InReplyTo(s.to_string()))
    }

    fn display(&self) -> HeaderValue {
        let name = HeaderName::new_from_ascii_str("In-Reply-To");
        HeaderValue::new(name, self.0.clone())
    }
}


use crate::templates::EmailTemplates;

#[derive(Debug, Clone)]
pub struct EmailService {
    transport: SmtpTransport,
    templates: EmailTemplates,
    smtp: SMTP,
    domains: Option<EmailDomains>,
}

impl EmailService {
    pub fn new(config: &Config, templates: EmailTemplates) -> Self {

        let smtp = config.smtp.clone();

        let credentials = Credentials::new(config.smtp.username.to_string(), config.smtp.password.to_string());

        let transport = SmtpTransport::relay(&config.smtp.server)
            .unwrap()
            .port(config.smtp.port) 
            .authentication(vec![Mechanism::Plain]) 
            .tls(Tls::None) 
            .timeout(Some(Duration::from_secs(10))) 
            .credentials(credentials)
            .build();

        Self {
            transport,
            templates,
            smtp,
            domains: config.email.domains.clone(),
        }
    }

    pub async fn send(&self, 
        recipient: &str,
        subject: &str,
        template_name: &str,
        template_data: Value,
    ) -> Result<(), anyhow::Error> {


        let html = self.templates.render(template_name, template_data)?;

        let text = html2text::from_read(html.as_bytes(), 80)?;

        let email = Message::builder()
            .from(self.smtp.account.parse()?)
            .to(recipient.parse()?)
            .subject(subject)
            .header(XPMMessageStream("outbound".to_string()))
            .multipart(
                MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(text),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(html),
                    ),
            )?;

        self.transport.send(&email)?;

        Ok(())
    }

    pub async fn send_reply(&self, 
        message_id: &str,
        recipient: &str,
        from: String,
        subject: &str,
        text: String,
        html: String,
    ) -> Result<(), anyhow::Error> {


        let email = Message::builder()
            .from(from.parse()?)
            .to(recipient.parse()?)
            .subject(subject)
            .header(XPMMessageStream("outbound".to_string()))
            .header(InReplyTo(message_id.to_string()))
            .multipart(
                MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(text),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(html),
                    ),
            )?;

        self.transport.send(&email)?;

        Ok(())
    }

    pub fn allowed(&self, email: &str) -> bool {
        if let Some(domains) = &self.domains {
            if let Some(allowed) = &domains.allow {
                for allowed_domain in allowed {
                    if email.ends_with(allowed_domain) {
                        tracing::info!("Email domain is allowed: {}", email);
                        return true;
                    }
                }
            }
            if let Some(reject) = &domains.reject {
                for reject_domain in reject {
                    if email.ends_with(reject_domain) {
                        tracing::info!("Email domain is rejected: {}", email);
                        return false;
                    }
                }
            }
        }
        tracing::info!("Email domain is allowed by default: {}", email);
        true
    }

}


