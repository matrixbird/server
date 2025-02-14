extern crate mailchecker;

use std::collections::HashSet;
use std::fs;
use serde::Deserialize;

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


