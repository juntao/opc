use crate::domain::OpcEvent;
use tokio::sync::broadcast;

/// Event bus for broadcasting system events to all listeners.
pub struct EventBus {
    sender: broadcast::Sender<OpcEvent>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Publish an event to all subscribers.
    pub fn publish(&self, event: OpcEvent) {
        // Ignore error if no receivers are listening
        let _ = self.sender.send(event);
    }

    /// Subscribe to receive events.
    pub fn subscribe(&self) -> broadcast::Receiver<OpcEvent> {
        self.sender.subscribe()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(1024)
    }
}
