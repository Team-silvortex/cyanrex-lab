//! Live queues exist only for subscribed owners. Receivers do not keep the registry alive.
use std::{
    collections::HashMap,
    sync::{Arc, OnceLock, RwLock, Weak},
};

use tokio::sync::broadcast::{self, error::RecvError};

use super::EventBus;
use crate::models::event::Event;

#[cfg(test)]
mod bench;
#[cfg(test)]
mod tests;

#[derive(Clone)]
pub(crate) struct UserFanout(Arc<Registry>);

struct Registry {
    channels: RwLock<HashMap<String, Arc<OwnerChannel>>>,
    capacity: usize,
}

struct OwnerChannel {
    sender: broadcast::Sender<Arc<SharedEvent>>,
}

#[derive(Debug)]
pub(crate) struct SharedEvent {
    event: Event,
    serialized: OnceLock<Result<String, serde_json::Error>>,
}

// Read-only access keeps the cached representation consistent with the underlying event.
impl std::ops::Deref for SharedEvent {
    type Target = Event;

    fn deref(&self) -> &Event {
        &self.event
    }
}

impl SharedEvent {
    pub(crate) fn json(&self) -> Result<&str, &serde_json::Error> {
        self.serialized
            .get_or_init(|| serde_json::to_string(&self.event))
            .as_deref()
    }
}

pub(crate) struct UserEventSubscription {
    receiver: Option<broadcast::Receiver<Arc<SharedEvent>>>,
    registry: Weak<Registry>,
    channel: Weak<OwnerChannel>,
    username: String,
}

impl EventBus {
    pub(crate) fn subscribe_for_user(&self, username: &str) -> UserEventSubscription {
        self.user_fanout.subscribe(username)
    }
}

impl UserFanout {
    pub(crate) fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "event channel capacity must be positive");
        Self(Arc::new(Registry {
            channels: RwLock::new(HashMap::new()),
            capacity,
        }))
    }

    pub(crate) fn subscribe(&self, username: &str) -> UserEventSubscription {
        let mut channels = self.0.channels.write().expect("event registry poisoned");
        let channel = channels.entry(username.to_owned()).or_insert_with(|| {
            Arc::new(OwnerChannel {
                sender: broadcast::channel(self.0.capacity).0,
            })
        });
        UserEventSubscription {
            receiver: Some(channel.sender.subscribe()),
            registry: Arc::downgrade(&self.0),
            channel: Arc::downgrade(channel),
            username: username.to_owned(),
        }
    }

    pub(crate) fn publish(&self, event: &Event) {
        let channel = self
            .0
            .channels
            .read()
            .expect("event registry poisoned")
            .get(&event.username)
            .cloned();
        // Release the registry lock before allocating the payload or waking subscribers.
        // Publishing to an owner without subscribers never allocates a channel or event clone.
        if let Some(channel) = channel {
            let _ = channel.sender.send(Arc::new(SharedEvent {
                event: event.clone(),
                serialized: OnceLock::new(),
            }));
        }
    }

    #[cfg(test)]
    pub(crate) fn active_owner_count(&self) -> usize {
        self.0.channels.read().unwrap().len()
    }

    #[cfg(test)]
    pub(crate) fn receiver_count(&self) -> usize {
        self.0
            .channels
            .read()
            .unwrap()
            .values()
            .map(|channel| channel.sender.receiver_count())
            .sum()
    }
}

impl UserEventSubscription {
    pub(crate) async fn recv(&mut self) -> Result<Arc<SharedEvent>, RecvError> {
        self.receiver
            .as_mut()
            .expect("live subscription")
            .recv()
            .await
    }

    #[cfg(test)]
    fn try_recv(&mut self) -> Result<Arc<SharedEvent>, broadcast::error::TryRecvError> {
        self.receiver
            .as_mut()
            .expect("live subscription")
            .try_recv()
    }
}

impl Drop for UserEventSubscription {
    fn drop(&mut self) {
        // Decrement the receiver count before deciding whether this was the last connection.
        drop(self.receiver.take());
        let Some(registry) = self.registry.upgrade() else {
            return;
        };
        let mut channels = registry.channels.write().expect("event registry poisoned");
        let unused = channels.get(&self.username).is_some_and(|channel| {
            // Another drop/subscribe may have replaced the channel while we waited for the lock.
            self.channel.ptr_eq(&Arc::downgrade(channel)) && channel.sender.receiver_count() == 0
        });
        let removed = unused.then(|| channels.remove(&self.username));
        drop(channels);
        // Destroy a potentially full queue outside the registry lock.
        drop(removed);
    }
}
