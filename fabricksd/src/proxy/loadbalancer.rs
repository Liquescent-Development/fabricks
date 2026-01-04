//! Load balancing strategies for routing requests to service instances.
//!
//! Provides various algorithms for distributing requests across multiple
//! instances of a service.

use std::sync::atomic::{AtomicUsize, Ordering};

/// Load balancing strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Strategy {
    /// Round-robin distribution across instances.
    #[default]
    RoundRobin,

    /// Send all requests to the first available instance.
    FirstAvailable,

    /// Random selection among available instances.
    Random,
}

/// Load balancer for distributing requests across instances.
#[derive(Debug)]
pub struct LoadBalancer {
    /// The load balancing strategy.
    strategy: Strategy,

    /// Counter for round-robin distribution.
    counter: AtomicUsize,
}

impl LoadBalancer {
    /// Creates a new load balancer with the given strategy.
    #[must_use]
    pub const fn new(strategy: Strategy) -> Self {
        Self {
            strategy,
            counter: AtomicUsize::new(0),
        }
    }

    /// Creates a new round-robin load balancer.
    #[must_use]
    pub const fn round_robin() -> Self {
        Self::new(Strategy::RoundRobin)
    }

    /// Selects an instance index from the available instances.
    ///
    /// # Arguments
    ///
    /// * `instance_count` - The number of available instances
    ///
    /// # Returns
    ///
    /// The index of the selected instance, or `None` if no instances available.
    #[must_use]
    pub fn select(&self, instance_count: usize) -> Option<usize> {
        if instance_count == 0 {
            return None;
        }

        match self.strategy {
            Strategy::RoundRobin => {
                let idx = self.counter.fetch_add(1, Ordering::Relaxed);
                Some(idx % instance_count)
            }
            Strategy::FirstAvailable => Some(0),
            Strategy::Random => {
                // Use a simple pseudo-random based on counter for determinism in tests
                let idx = self.counter.fetch_add(1, Ordering::Relaxed);
                Some((idx.wrapping_mul(1_103_515_245).wrapping_add(12345)) % instance_count)
            }
        }
    }

    /// Returns the current strategy.
    #[must_use]
    pub const fn strategy(&self) -> Strategy {
        self.strategy
    }
}

impl Default for LoadBalancer {
    fn default() -> Self {
        Self::round_robin()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_robin() {
        let lb = LoadBalancer::round_robin();

        // With 3 instances, should cycle through 0, 1, 2, 0, 1, 2...
        assert_eq!(lb.select(3), Some(0));
        assert_eq!(lb.select(3), Some(1));
        assert_eq!(lb.select(3), Some(2));
        assert_eq!(lb.select(3), Some(0));
    }

    #[test]
    fn test_first_available() {
        let lb = LoadBalancer::new(Strategy::FirstAvailable);

        // Should always return 0
        assert_eq!(lb.select(3), Some(0));
        assert_eq!(lb.select(3), Some(0));
        assert_eq!(lb.select(3), Some(0));
    }

    #[test]
    fn test_no_instances() {
        let lb = LoadBalancer::round_robin();

        // Should return None when no instances
        assert_eq!(lb.select(0), None);
    }

    #[test]
    fn test_single_instance() {
        let lb = LoadBalancer::round_robin();

        // With 1 instance, should always return 0
        assert_eq!(lb.select(1), Some(0));
        assert_eq!(lb.select(1), Some(0));
        assert_eq!(lb.select(1), Some(0));
    }

    #[test]
    fn test_random_distribution() {
        let lb = LoadBalancer::new(Strategy::Random);

        // Should return valid indices
        for _ in 0..10 {
            let idx = lb.select(3);
            assert!(idx.is_some());
            assert!(idx.expect("has value") < 3);
        }
    }
}
