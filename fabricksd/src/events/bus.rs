//! Event bus for pub/sub messaging.

use std::collections::VecDeque;

use tokio::sync::{RwLock, mpsc};

use super::types::{Event, EventType};

/// Event bus for publishing and subscribing to daemon events.
///
/// The event bus provides a simple pub/sub mechanism for daemon events.
/// Subscribers receive events through async channels, and the bus maintains
/// a ring buffer of recent events for history queries.
pub struct EventBus {
    /// Active subscribers.
    subscribers: RwLock<Vec<mpsc::Sender<Event>>>,

    /// Event history (ring buffer).
    history: RwLock<VecDeque<Event>>,

    /// Maximum history size.
    max_history: usize,

    /// Channel buffer size for new subscribers.
    buffer_size: usize,
}

impl EventBus {
    /// Creates a new event bus.
    ///
    /// # Arguments
    ///
    /// * `buffer_size` - The buffer size for subscriber channels
    /// * `max_history` - The maximum number of events to retain in history
    #[must_use]
    pub fn new(buffer_size: usize, max_history: usize) -> Self {
        Self {
            subscribers: RwLock::new(Vec::new()),
            history: RwLock::new(VecDeque::with_capacity(max_history)),
            max_history,
            buffer_size,
        }
    }

    /// Publishes an event to all subscribers.
    ///
    /// The event is added to the history and sent to all active subscribers.
    /// Subscribers with full or closed channels are automatically removed.
    pub async fn publish(&self, event: Event) {
        // Add to history
        {
            let mut history = self.history.write().await;
            if history.len() >= self.max_history {
                history.pop_front();
            }
            history.push_back(event.clone());
        }

        // Send to all subscribers, remove closed channels
        let mut subscribers = self.subscribers.write().await;
        subscribers.retain(|tx| tx.try_send(event.clone()).is_ok());
    }

    /// Subscribes to all events.
    ///
    /// Returns a receiver channel that will receive all published events.
    pub async fn subscribe(&self) -> mpsc::Receiver<Event> {
        let (tx, rx) = mpsc::channel(self.buffer_size);
        self.subscribers.write().await.push(tx);
        rx
    }

    /// Subscribes to events matching the given filter.
    ///
    /// Returns a receiver channel that will only receive events of the
    /// specified types.
    ///
    /// # Arguments
    ///
    /// * `filter` - The event types to subscribe to
    pub async fn subscribe_filtered(&self, filter: Vec<EventType>) -> mpsc::Receiver<Event> {
        let (outer_tx, outer_rx) = mpsc::channel(self.buffer_size);
        let mut inner_rx = self.subscribe().await;

        tokio::spawn(async move {
            while let Some(event) = inner_rx.recv().await {
                let should_send = filter.contains(&event.event_type);
                if should_send && outer_tx.send(event).await.is_err() {
                    break;
                }
            }
        });

        outer_rx
    }

    /// Gets event history.
    ///
    /// Returns events in reverse chronological order (newest first).
    ///
    /// # Arguments
    ///
    /// * `limit` - Optional limit on the number of events to return
    pub async fn history(&self, limit: Option<usize>) -> Vec<Event> {
        let history = self.history.read().await;
        let iter = history.iter().rev();
        match limit {
            Some(n) => iter.take(n).cloned().collect(),
            None => iter.cloned().collect(),
        }
    }

    /// Gets the number of active subscribers.
    pub async fn subscriber_count(&self) -> usize {
        self.subscribers.read().await.len()
    }

    /// Clears all event history.
    pub async fn clear_history(&self) {
        self.history.write().await.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_publish_and_subscribe() {
        let bus = EventBus::new(10, 100);

        let mut rx = bus.subscribe().await;
        assert_eq!(bus.subscriber_count().await, 1);

        let event = Event::new(
            EventType::DaemonStarted,
            serde_json::json!({
                "version": "1.0.0"
            }),
        );

        bus.publish(event.clone()).await;

        let received = tokio::time::timeout(Duration::from_millis(100), rx.recv())
            .await
            .expect("should not timeout")
            .expect("should receive event");

        assert_eq!(received.id, event.id);
        assert_eq!(received.event_type, EventType::DaemonStarted);
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let bus = EventBus::new(10, 100);

        let mut rx1 = bus.subscribe().await;
        let mut rx2 = bus.subscribe().await;
        assert_eq!(bus.subscriber_count().await, 2);

        let event = Event::empty(EventType::ServiceCreated);
        bus.publish(event.clone()).await;

        let received1 = rx1.recv().await.expect("should receive");
        let received2 = rx2.recv().await.expect("should receive");

        assert_eq!(received1.id, event.id);
        assert_eq!(received2.id, event.id);
    }

    #[tokio::test]
    async fn test_filtered_subscription() {
        let bus = EventBus::new(10, 100);

        let mut rx = bus
            .subscribe_filtered(vec![EventType::ServiceStarted, EventType::ServiceStopped])
            .await;

        // Should not receive this
        bus.publish(Event::empty(EventType::DaemonStarted)).await;
        // Should receive this
        bus.publish(Event::empty(EventType::ServiceStarted)).await;
        // Should not receive this
        bus.publish(Event::empty(EventType::NetworkCreated)).await;
        // Should receive this
        bus.publish(Event::empty(EventType::ServiceStopped)).await;

        let received1 = tokio::time::timeout(Duration::from_millis(100), rx.recv())
            .await
            .expect("should not timeout")
            .expect("should receive");
        assert_eq!(received1.event_type, EventType::ServiceStarted);

        let received2 = tokio::time::timeout(Duration::from_millis(100), rx.recv())
            .await
            .expect("should not timeout")
            .expect("should receive");
        assert_eq!(received2.event_type, EventType::ServiceStopped);
    }

    #[tokio::test]
    async fn test_history() {
        let bus = EventBus::new(10, 100);

        bus.publish(Event::empty(EventType::ServiceCreated)).await;
        bus.publish(Event::empty(EventType::ServiceStarted)).await;
        bus.publish(Event::empty(EventType::ServiceStopped)).await;

        let history = bus.history(None).await;
        assert_eq!(history.len(), 3);
        // Newest first
        assert_eq!(history[0].event_type, EventType::ServiceStopped);
        assert_eq!(history[1].event_type, EventType::ServiceStarted);
        assert_eq!(history[2].event_type, EventType::ServiceCreated);
    }

    #[tokio::test]
    async fn test_history_limit() {
        let bus = EventBus::new(10, 100);

        for _ in 0..10 {
            bus.publish(Event::empty(EventType::ServiceCreated)).await;
        }

        let history = bus.history(Some(5)).await;
        assert_eq!(history.len(), 5);
    }

    #[tokio::test]
    async fn test_history_ring_buffer() {
        let bus = EventBus::new(10, 5); // max 5 events

        for i in 0..10 {
            bus.publish(Event::new(
                EventType::ServiceCreated,
                serde_json::json!({
                    "index": i
                }),
            ))
            .await;
        }

        let history = bus.history(None).await;
        assert_eq!(history.len(), 5);
        // Should only have events 5-9 (newest)
        assert_eq!(history[0].data["index"], 9);
        assert_eq!(history[4].data["index"], 5);
    }

    #[tokio::test]
    async fn test_subscriber_cleanup_on_channel_close() {
        let bus = EventBus::new(10, 100);

        let rx = bus.subscribe().await;
        assert_eq!(bus.subscriber_count().await, 1);

        // Drop the receiver
        drop(rx);

        // Publish an event - this should clean up the dead subscriber
        bus.publish(Event::empty(EventType::DaemonStarted)).await;

        assert_eq!(bus.subscriber_count().await, 0);
    }

    #[tokio::test]
    async fn test_clear_history() {
        let bus = EventBus::new(10, 100);

        bus.publish(Event::empty(EventType::ServiceCreated)).await;
        bus.publish(Event::empty(EventType::ServiceStarted)).await;
        assert_eq!(bus.history(None).await.len(), 2);

        bus.clear_history().await;
        assert_eq!(bus.history(None).await.len(), 0);
    }
}
