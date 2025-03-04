use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub struct FallbackManager {
    pub error_count: Arc<Mutex<u32>>
}

impl FallbackManager {
    pub fn new() -> Self {
        Self { error_count: Arc::new(Mutex::new(0)) }
    }

    pub fn log_error(&self) {
        let mut count = self.error_count.lock().unwrap();
        *count += 1;
        println!("Fallback triggered. Error count: {}", *count);
    }
}