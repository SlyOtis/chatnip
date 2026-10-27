use std::sync::{Arc, RwLock};
use std::collections::HashMap;

#[derive(Clone)]
pub struct LoadingStateManager {
    states: Arc<RwLock<HashMap<String, usize>>>,
}

impl LoadingStateManager {
    pub fn new() -> Self {
        Self {
            states: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn is_loading(&self, key: &str) -> bool {
        let states = self.states.read().unwrap();
        states.get(key).copied().unwrap_or(0) > 0
    }

    pub fn start_loading(&self, key: &str) {
        let mut states = self.states.write().unwrap();
        let counter = states.entry(key.to_string()).or_insert(0);
        *counter += 1;
    }

    pub fn finish_loading(&self, key: &str) {
        let mut states = self.states.write().unwrap();
        if let Some(counter) = states.get_mut(key) {
            if *counter > 0 {
                *counter -= 1;
            }
        }
    }
    
}

pub fn loading_key_conversations() -> &'static str {
    "conversations"
}

pub fn loading_key_current_conversation() -> &'static str {
    "current_conversation"
}