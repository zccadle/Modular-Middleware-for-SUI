use anyhow::{Result, anyhow};
use std::time::{Duration, Instant};
use std::sync::{Arc, Mutex};
use serde_json::Value;
use reqwest;

// Simple utility function that replaces cached API call functionality
pub async fn cached_api_call(url: &str) -> Result<Value> {
    // We're not using the cached crate anymore to simplify
    // Instead, using a simple direct call
    let client = reqwest::Client::new();
    let resp = client.get(url).send().await?;
    
    if resp.status().is_success() {
        let json = resp.json::<Value>().await?;
        Ok(json)
    } else {
        Err(anyhow!("API call failed with status: {}", resp.status()))
    }
}

// Simplified OAuth client that doesn't use the complex oauth2 crate
#[allow(dead_code)]
pub struct OAuthClient {
    client_id: String,
    client_secret: String,
    token: Arc<Mutex<Option<String>>>,
    token_expires: Arc<Mutex<Option<Instant>>>,
}

#[allow(dead_code)]
impl OAuthClient {
    pub fn new(client_id: &str, client_secret: &str) -> Self {
        Self {
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            token: Arc::new(Mutex::new(None)),
            token_expires: Arc::new(Mutex::new(None)),
        }
    }
    
    // Simplified token acquisition (for demo purposes)
    pub async fn get_token(&self, auth_code: &str) -> Result<()> {
        // This is a dummy implementation that just sets a token
        // without actually making the OAuth request
        println!("Getting token with auth code: {}", auth_code);
        
        let token = format!("dummy_token_{}", auth_code);
        let expires_in = Duration::from_secs(3600); // 1 hour
        
        let mut token_guard = self.token.lock().unwrap();
        *token_guard = Some(token);
        
        let mut expires_guard = self.token_expires.lock().unwrap();
        *expires_guard = Some(Instant::now() + expires_in);
        
        Ok(())
    }
    
    pub fn is_token_valid(&self) -> bool {
        let expires = self.token_expires.lock().unwrap();
        
        if let Some(expiry) = *expires {
            Instant::now() < expiry
        } else {
            false
        }
    }
    
    pub async fn make_authenticated_request(&self, url: &str) -> Result<Value> {
        if !self.is_token_valid() {
            return Err(anyhow!("OAuth token not valid or expired"));
        }
        
        let token = {
            let token_guard = self.token.lock().unwrap();
            match &*token_guard {
                Some(t) => t.clone(),
                None => return Err(anyhow!("No OAuth token available")),
            }
        };
        
        let client = reqwest::Client::new();
        let resp = client.get(url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await?;
            
        if resp.status().is_success() {
            let json = resp.json::<Value>().await?;
            Ok(json)
        } else {
            Err(anyhow!("API call failed with status: {}", resp.status()))
        }
    }
}