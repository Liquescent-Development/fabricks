//! Persistent state storage using sled.

use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde::Serialize;
use sled::Db;

use crate::error::Result;

/// Well-known tree names for different entity types.
///
/// These constants are used in later phases for organizing data in the store.
#[allow(dead_code)]
pub mod trees {
    /// Tree for service state.
    pub const SERVICES: &str = "services";
    /// Tree for network state.
    pub const NETWORKS: &str = "networks";
    /// Tree for volume state.
    pub const VOLUMES: &str = "volumes";
    /// Tree for daemon metadata.
    pub const METADATA: &str = "metadata";
}

/// Persistent state store backed by sled.
///
/// Provides a generic key-value interface for storing serializable data
/// organized into separate trees (namespaces).
#[derive(Clone, Debug)]
pub struct StateStore {
    db: Arc<Db>,
}

impl StateStore {
    /// Creates a new state store wrapping the given database.
    #[must_use]
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// Stores a serializable value in the specified tree.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails or the database operation fails.
    pub fn put<T: Serialize>(&self, tree_name: &str, key: &str, value: &T) -> Result<()> {
        let tree = self.db.open_tree(tree_name)?;
        let json = serde_json::to_vec(value)?;
        tree.insert(key.as_bytes(), json)?;
        tree.flush()?;
        Ok(())
    }

    /// Retrieves a value by key from the specified tree.
    ///
    /// Returns `None` if the key does not exist.
    ///
    /// # Errors
    ///
    /// Returns an error if deserialization fails or the database operation fails.
    pub fn get<T: DeserializeOwned>(&self, tree_name: &str, key: &str) -> Result<Option<T>> {
        let tree = self.db.open_tree(tree_name)?;
        match tree.get(key.as_bytes())? {
            Some(bytes) => {
                let value = serde_json::from_slice(&bytes)?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    /// Deletes a value by key from the specified tree.
    ///
    /// Returns `true` if the key existed and was deleted, `false` otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error if the database operation fails.
    pub fn delete(&self, tree_name: &str, key: &str) -> Result<bool> {
        let tree = self.db.open_tree(tree_name)?;
        let existed = tree.remove(key.as_bytes())?.is_some();
        tree.flush()?;
        Ok(existed)
    }

    /// Lists all values in the specified tree.
    ///
    /// # Errors
    ///
    /// Returns an error if deserialization fails or the database operation fails.
    pub fn list<T: DeserializeOwned>(&self, tree_name: &str) -> Result<Vec<T>> {
        let tree = self.db.open_tree(tree_name)?;
        tree.iter().try_fold(Vec::new(), |mut results, item| {
            let (_, value) = item?;
            let parsed: T = serde_json::from_slice(&value)?;
            results.push(parsed);
            Ok(results)
        })
    }

    /// Lists all keys in the specified tree.
    ///
    /// # Errors
    ///
    /// Returns an error if the database operation fails.
    pub fn keys(&self, tree_name: &str) -> Result<Vec<String>> {
        let tree = self.db.open_tree(tree_name)?;
        tree.iter().try_fold(Vec::new(), |mut keys, item| {
            let (key, _) = item?;
            if let Ok(key_str) = String::from_utf8(key.to_vec()) {
                keys.push(key_str);
            }
            Ok(keys)
        })
    }

    /// Counts the number of entries in the specified tree.
    ///
    /// # Errors
    ///
    /// Returns an error if the database operation fails.
    pub fn count(&self, tree_name: &str) -> Result<usize> {
        let tree = self.db.open_tree(tree_name)?;
        Ok(tree.len())
    }

    /// Clears all entries in the specified tree.
    ///
    /// # Errors
    ///
    /// Returns an error if the database operation fails.
    pub fn clear(&self, tree_name: &str) -> Result<()> {
        let tree = self.db.open_tree(tree_name)?;
        tree.clear()?;
        tree.flush()?;
        Ok(())
    }

    /// Flushes all pending writes to disk.
    ///
    /// # Errors
    ///
    /// Returns an error if the flush operation fails.
    pub fn flush(&self) -> Result<()> {
        self.db.flush()?;
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Network convenience methods
    // -------------------------------------------------------------------------

    /// Saves a network state.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization or database operation fails.
    pub fn save_network(&self, network: &crate::network::NetworkState) -> Result<()> {
        self.put(trees::NETWORKS, &network.id, network)
    }

    /// Deletes a network state.
    ///
    /// # Errors
    ///
    /// Returns an error if the database operation fails.
    pub fn delete_network(&self, id: &str) -> Result<bool> {
        self.delete(trees::NETWORKS, id)
    }

    /// Gets a network state by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if deserialization or database operation fails.
    pub fn get_network(&self, id: &str) -> Result<Option<crate::network::NetworkState>> {
        self.get(trees::NETWORKS, id)
    }

    /// Lists all network states.
    ///
    /// # Errors
    ///
    /// Returns an error if deserialization or database operation fails.
    pub fn list_networks(&self) -> Result<Vec<crate::network::NetworkState>> {
        self.list(trees::NETWORKS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use tempfile::tempdir;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestData {
        id: String,
        value: i32,
    }

    fn create_test_store() -> (StateStore, tempfile::TempDir) {
        let dir = tempdir().expect("should create temp dir");
        let db = sled::open(dir.path()).expect("should open db");
        let store = StateStore::new(Arc::new(db));
        (store, dir)
    }

    #[test]
    fn test_put_and_get() {
        let (store, _dir) = create_test_store();

        let data = TestData {
            id: "test-1".to_string(),
            value: 42,
        };

        store.put("test_tree", "key1", &data).expect("should put");

        let retrieved: Option<TestData> = store.get("test_tree", "key1").expect("should get");
        assert_eq!(retrieved, Some(data));
    }

    #[test]
    fn test_get_nonexistent() {
        let (store, _dir) = create_test_store();

        let retrieved: Option<TestData> = store.get("test_tree", "nonexistent").expect("should get");
        assert_eq!(retrieved, None);
    }

    #[test]
    fn test_delete() {
        let (store, _dir) = create_test_store();

        let data = TestData {
            id: "test-1".to_string(),
            value: 42,
        };

        store.put("test_tree", "key1", &data).expect("should put");

        let deleted = store.delete("test_tree", "key1").expect("should delete");
        assert!(deleted);

        let retrieved: Option<TestData> = store.get("test_tree", "key1").expect("should get");
        assert_eq!(retrieved, None);
    }

    #[test]
    fn test_delete_nonexistent() {
        let (store, _dir) = create_test_store();

        let deleted = store.delete("test_tree", "nonexistent").expect("should delete");
        assert!(!deleted);
    }

    #[test]
    fn test_list() {
        let (store, _dir) = create_test_store();

        let data1 = TestData {
            id: "test-1".to_string(),
            value: 1,
        };
        let data2 = TestData {
            id: "test-2".to_string(),
            value: 2,
        };

        store.put("test_tree", "key1", &data1).expect("should put");
        store.put("test_tree", "key2", &data2).expect("should put");

        let items: Vec<TestData> = store.list("test_tree").expect("should list");
        assert_eq!(items.len(), 2);
        assert!(items.contains(&data1));
        assert!(items.contains(&data2));
    }

    #[test]
    fn test_keys() {
        let (store, _dir) = create_test_store();

        let data = TestData {
            id: "test-1".to_string(),
            value: 1,
        };

        store.put("test_tree", "key1", &data).expect("should put");
        store.put("test_tree", "key2", &data).expect("should put");

        let keys = store.keys("test_tree").expect("should get keys");
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"key1".to_string()));
        assert!(keys.contains(&"key2".to_string()));
    }

    #[test]
    fn test_count() {
        let (store, _dir) = create_test_store();

        assert_eq!(store.count("test_tree").expect("should count"), 0);

        let data = TestData {
            id: "test-1".to_string(),
            value: 1,
        };

        store.put("test_tree", "key1", &data).expect("should put");
        assert_eq!(store.count("test_tree").expect("should count"), 1);

        store.put("test_tree", "key2", &data).expect("should put");
        assert_eq!(store.count("test_tree").expect("should count"), 2);
    }

    #[test]
    fn test_clear() {
        let (store, _dir) = create_test_store();

        let data = TestData {
            id: "test-1".to_string(),
            value: 1,
        };

        store.put("test_tree", "key1", &data).expect("should put");
        store.put("test_tree", "key2", &data).expect("should put");
        assert_eq!(store.count("test_tree").expect("should count"), 2);

        store.clear("test_tree").expect("should clear");
        assert_eq!(store.count("test_tree").expect("should count"), 0);
    }

    #[test]
    fn test_separate_trees() {
        let (store, _dir) = create_test_store();

        let data1 = TestData {
            id: "test-1".to_string(),
            value: 1,
        };
        let data2 = TestData {
            id: "test-2".to_string(),
            value: 2,
        };

        store.put("tree_a", "key1", &data1).expect("should put");
        store.put("tree_b", "key1", &data2).expect("should put");

        let from_a: Option<TestData> = store.get("tree_a", "key1").expect("should get");
        let from_b: Option<TestData> = store.get("tree_b", "key1").expect("should get");

        assert_eq!(from_a, Some(data1));
        assert_eq!(from_b, Some(data2));
    }
}
