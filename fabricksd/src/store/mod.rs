//! Persistent state storage module.
//!
//! This module provides the [`StateStore`] for persisting daemon state
//! to a sled embedded database.

mod state_store;

pub use state_store::StateStore;
