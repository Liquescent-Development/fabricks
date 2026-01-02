//! Event type definitions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Event categories for daemon operations.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    // Daemon lifecycle events
    /// Daemon has started.
    DaemonStarted,
    /// Daemon is stopping.
    DaemonStopping,
    /// Daemon configuration was reloaded.
    DaemonConfigReloaded,

    // Service lifecycle events
    /// A service was created.
    ServiceCreated,
    /// A service was started.
    ServiceStarted,
    /// A service was stopped.
    ServiceStopped,
    /// A service failed.
    ServiceFailed,
    /// A service was scaled.
    ServiceScaled,
    /// A service was deleted.
    ServiceDeleted,

    // Health events
    /// Service health status changed.
    HealthChanged,

    // Network events
    /// A network was created.
    NetworkCreated,
    /// A network was deleted.
    NetworkDeleted,

    // Volume events
    /// A volume was created.
    VolumeCreated,
    /// A volume was deleted.
    VolumeDeleted,
}

/// A daemon event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Unique event ID.
    pub id: Uuid,

    /// Event type.
    pub event_type: EventType,

    /// When the event occurred.
    pub timestamp: DateTime<Utc>,

    /// Event-specific data.
    pub data: serde_json::Value,
}

impl Event {
    /// Creates a new event with the given type and data.
    ///
    /// The event ID and timestamp are automatically generated.
    pub fn new<T: Serialize>(event_type: EventType, data: T) -> Self {
        Self {
            id: Uuid::new_v4(),
            event_type,
            timestamp: Utc::now(),
            data: serde_json::to_value(data).unwrap_or(serde_json::Value::Null),
        }
    }

    /// Creates a new event with no data.
    #[must_use]
    pub fn empty(event_type: EventType) -> Self {
        Self {
            id: Uuid::new_v4(),
            event_type,
            timestamp: Utc::now(),
            data: serde_json::Value::Null,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_creation() {
        let event = Event::new(EventType::DaemonStarted, serde_json::json!({
            "version": "1.0.0"
        }));

        assert_eq!(event.event_type, EventType::DaemonStarted);
        assert!(!event.id.is_nil());
        assert_eq!(event.data["version"], "1.0.0");
    }

    #[test]
    fn test_event_empty() {
        let event = Event::empty(EventType::DaemonStopping);

        assert_eq!(event.event_type, EventType::DaemonStopping);
        assert!(event.data.is_null());
    }

    #[test]
    fn test_event_serialization() {
        let event = Event::new(EventType::ServiceCreated, serde_json::json!({
            "service_id": "test-service"
        }));

        let json = serde_json::to_string(&event).expect("should serialize");
        let parsed: Event = serde_json::from_str(&json).expect("should parse");

        assert_eq!(parsed.id, event.id);
        assert_eq!(parsed.event_type, event.event_type);
        assert_eq!(parsed.data["service_id"], "test-service");
    }

    #[test]
    fn test_event_type_serialization() {
        let event_type = EventType::ServiceStarted;
        let json = serde_json::to_string(&event_type).expect("should serialize");
        assert_eq!(json, "\"service_started\"");

        let parsed: EventType = serde_json::from_str(&json).expect("should parse");
        assert_eq!(parsed, EventType::ServiceStarted);
    }
}
