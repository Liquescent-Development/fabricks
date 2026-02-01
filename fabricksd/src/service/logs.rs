//! Per-service log buffer for capturing WASM stdout/stderr.
//!
//! Provides a bounded ring buffer that captures log output from WASM service
//! instances. Each entry is timestamped and tagged with the stream (stdout/stderr).

use std::collections::VecDeque;
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use fabricks_runtime::output::LogStream;

/// Default maximum number of log entries per service.
const DEFAULT_CAPACITY: usize = 10_000;

/// A single log entry with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Timestamp when the line was captured.
    pub timestamp: DateTime<Utc>,
    /// Which stream this came from.
    pub stream: LogStream,
    /// The log line content.
    pub message: String,
}

/// Thread-safe bounded ring buffer for service logs.
///
/// Uses a `VecDeque` behind a `Mutex` with a configurable max capacity.
/// When the buffer is full, the oldest entries are evicted.
///
/// Uses `std::sync::Mutex` (not `tokio::sync::Mutex`) because the critical
/// section is tiny (push to `VecDeque`) and never awaits inside.
pub struct ServiceLogBuffer {
    /// The bounded buffer.
    entries: Mutex<VecDeque<LogEntry>>,
    /// Maximum number of entries retained.
    capacity: usize,
}

impl std::fmt::Debug for ServiceLogBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let len = self
            .entries
            .lock()
            .map(|e| e.len())
            .unwrap_or(0);
        f.debug_struct("ServiceLogBuffer")
            .field("capacity", &self.capacity)
            .field("len", &len)
            .finish()
    }
}

impl Default for ServiceLogBuffer {
    fn default() -> Self {
        Self::new(DEFAULT_CAPACITY)
    }
}

impl ServiceLogBuffer {
    /// Creates a new log buffer with the given capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Mutex::new(VecDeque::with_capacity(capacity.min(1024))),
            capacity,
        }
    }

    /// Appends a log entry, evicting the oldest if at capacity.
    pub fn push(&self, stream: LogStream, message: String) {
        let entry = LogEntry {
            timestamp: Utc::now(),
            stream,
            message,
        };

        if let Ok(mut entries) = self.entries.lock() {
            if entries.len() >= self.capacity {
                entries.pop_front();
            }
            entries.push_back(entry);
        }
    }

    /// Returns log entries, optionally limited to the last N.
    pub fn entries(&self, tail: Option<usize>) -> Vec<LogEntry> {
        let Ok(entries) = self.entries.lock() else {
            return Vec::new();
        };

        match tail {
            Some(n) => entries.iter().rev().take(n).rev().cloned().collect(),
            None => entries.iter().cloned().collect(),
        }
    }

    /// Clears all log entries.
    pub fn clear(&self) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.clear();
        }
    }
}

impl fabricks_runtime::output::LogWriter for ServiceLogBuffer {
    fn write_log(&self, stream: LogStream, message: &str) {
        self.push(stream, message.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_and_retrieve() {
        let buffer = ServiceLogBuffer::new(100);
        buffer.push(LogStream::Stdout, "hello".to_string());
        buffer.push(LogStream::Stderr, "error".to_string());

        let entries = buffer.entries(None);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].message, "hello");
        assert_eq!(entries[0].stream, LogStream::Stdout);
        assert_eq!(entries[1].message, "error");
        assert_eq!(entries[1].stream, LogStream::Stderr);
    }

    #[test]
    fn test_tail() {
        let buffer = ServiceLogBuffer::new(100);
        for i in 0..10 {
            buffer.push(LogStream::Stdout, format!("line {i}"));
        }

        let entries = buffer.entries(Some(3));
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].message, "line 7");
        assert_eq!(entries[1].message, "line 8");
        assert_eq!(entries[2].message, "line 9");
    }

    #[test]
    fn test_capacity_eviction() {
        let buffer = ServiceLogBuffer::new(5);
        for i in 0..10 {
            buffer.push(LogStream::Stdout, format!("line {i}"));
        }

        let entries = buffer.entries(None);
        assert_eq!(entries.len(), 5);
        assert_eq!(entries[0].message, "line 5");
        assert_eq!(entries[4].message, "line 9");
    }

    #[test]
    fn test_clear() {
        let buffer = ServiceLogBuffer::new(100);
        buffer.push(LogStream::Stdout, "hello".to_string());
        assert_eq!(buffer.entries(None).len(), 1);

        buffer.clear();
        assert!(buffer.entries(None).is_empty());
    }
}
