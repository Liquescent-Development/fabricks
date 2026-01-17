//! Resource limiting for WASM modules.
//!
//! Provides memory and table size limits configuration.
//! These limits are enforced at the WASM level using Wasmtime's `StoreLimits`,
//! preventing modules from exceeding their allocated resources.

/// Default maximum memory (256 MB).
pub const DEFAULT_MAX_MEMORY_BYTES: usize = 256 * 1024 * 1024;

/// Default maximum table elements (10,000).
pub const DEFAULT_MAX_TABLE_ELEMENTS: usize = 10_000;

/// Resource limits configuration for WASM modules.
///
/// Specifies the maximum resources a WASM module can consume.
/// These limits are enforced via Wasmtime's `ResourceLimiter` trait.
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Maximum linear memory in bytes.
    /// Defaults to 256 MB if not specified.
    pub max_memory_bytes: usize,

    /// Maximum number of table elements.
    /// Defaults to 10,000 if not specified.
    pub max_table_elements: usize,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_bytes: DEFAULT_MAX_MEMORY_BYTES,
            max_table_elements: DEFAULT_MAX_TABLE_ELEMENTS,
        }
    }
}

impl ResourceLimits {
    /// Creates resource limits with the specified memory limit.
    #[must_use]
    pub const fn with_memory(max_memory_bytes: usize) -> Self {
        Self {
            max_memory_bytes,
            max_table_elements: DEFAULT_MAX_TABLE_ELEMENTS,
        }
    }

    /// Creates resource limits with the specified table limit.
    #[must_use]
    pub const fn with_table(max_table_elements: usize) -> Self {
        Self {
            max_memory_bytes: DEFAULT_MAX_MEMORY_BYTES,
            max_table_elements,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_limits() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.max_memory_bytes, DEFAULT_MAX_MEMORY_BYTES);
        assert_eq!(limits.max_table_elements, DEFAULT_MAX_TABLE_ELEMENTS);
    }

    #[test]
    fn test_with_memory() {
        let limits = ResourceLimits::with_memory(64 * 1024 * 1024);
        assert_eq!(limits.max_memory_bytes, 64 * 1024 * 1024);
        assert_eq!(limits.max_table_elements, DEFAULT_MAX_TABLE_ELEMENTS);
    }

    #[test]
    fn test_with_table() {
        let limits = ResourceLimits::with_table(5000);
        assert_eq!(limits.max_memory_bytes, DEFAULT_MAX_MEMORY_BYTES);
        assert_eq!(limits.max_table_elements, 5000);
    }
}
