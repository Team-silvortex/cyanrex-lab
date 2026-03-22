use std::sync::Arc;

use tokio::sync::{broadcast, RwLock};

use crate::models::event::Event;

#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<Event>,
    history: Arc<RwLock<Vec<Event>>>,
}

impl EventBus {
    pub fn new(buffer: usize) -> Self {
        let (sender, _) = broadcast::channel(buffer);
        Self {
            sender,
            history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn publish(&self, event: Event) {
        let _ = self.sender.send(event.clone());
        self.history.write().await.push(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.sender.subscribe()
    }

    pub fn snapshot(&self) -> Vec<Event> {
        self.history.blocking_read().clone()
    }
}
