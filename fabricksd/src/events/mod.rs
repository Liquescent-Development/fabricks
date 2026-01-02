//! Event system for pub/sub messaging.
//!
//! This module provides the [`EventBus`] for publishing and subscribing
//! to daemon events, as well as event type definitions.

mod bus;
mod types;

pub use bus::EventBus;
pub use types::{Event, EventType};
