use anyhow::{Result, anyhow};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use serde_json::Value;
use websocket::client::ClientBuilder;
use websocket::OwnedMessage;

// WebSocket client for real-time data
pub struct WebSocketClient {
    url: String,
    last_message: Arc<Mutex<Option<Value>>>,
    connected: Arc<Mutex<bool>>,
}

impl WebSocketClient {
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_string(),
            last_message: Arc::new(Mutex::new(None)),
            connected: Arc::new(Mutex::new(false)),
        }
    }
    
    pub fn start_listening(&self) -> Result<()> {
        let url = self.url.clone();
        let last_message = self.last_message.clone();
        let connected = self.connected.clone();
        
        std::thread::spawn(move || {
            loop {
                match ClientBuilder::new(&url)
                    .expect("Invalid WebSocket URL")
                    .connect_insecure() 
                {
                    Ok(mut client) => {
                        {
                            let mut is_connected = connected.lock().unwrap();
                            *is_connected = true;
                        }
                        
                        println!("WebSocket connected to {}", url);
                        
                        for message in client.incoming_messages() {
                            match message {
                                Ok(OwnedMessage::Text(text)) => {
                                    match serde_json::from_str::<Value>(&text) {
                                        Ok(json) => {
                                            let mut msg = last_message.lock().unwrap();
                                            *msg = Some(json);
                                        },
                                        Err(e) => println!("Failed to parse WebSocket message: {}", e),
                                    }
                                },
                                Ok(OwnedMessage::Close(_)) => {
                                    println!("WebSocket connection closed");
                                    break;
                                },
                                Err(e) => {
                                    println!("WebSocket error: {}", e);
                                    break;
                                },
                                _ => {}
                            }
                        }
                        
                        {
                            let mut is_connected = connected.lock().unwrap();
                            *is_connected = false;
                        }
                    },
                    Err(e) => {
                        println!("Failed to connect to WebSocket: {}", e);
                        std::thread::sleep(Duration::from_secs(5));
                    }
                }
                
                // Retry after a delay
                std::thread::sleep(Duration::from_secs(5));
            }
        });
        
        Ok(())
    }
    
    pub fn get_last_message(&self) -> Option<Value> {
        let msg = self.last_message.lock().unwrap();
        msg.clone()
    }
    
    pub fn is_connected(&self) -> bool {
        let connected = self.connected.lock().unwrap();
        *connected
    }
    
    pub fn send_message(&self, message: &str) -> Result<()> {
        if !self.is_connected() {
            return Err(anyhow!("WebSocket not connected"));
        }
        
        let mut client = ClientBuilder::new(&self.url)
            .expect("Invalid WebSocket URL")
            .connect_insecure()?;
            
        client.send_message(&OwnedMessage::Text(message.to_string()))?;
        Ok(())
    }
}