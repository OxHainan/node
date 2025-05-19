use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// State operation types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StateOperation {
    /// Insert or update a key with a value
    Insert { key: String, value: String },
    /// Delete a key
    Delete { key: String },
}

/// Represents a set of changes to the blockchain state
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StateDiff {
    /// Previous state root hash
    pub prev_root: String,
    /// New state root hash after changes
    pub new_root: String,
    /// List of operations to apply
    pub operations: Vec<StateOperation>,
}

impl StateDiff {
    /// Create a new state diff with the given root hash
    pub fn new(prev_root: String) -> Self {
        Self {
            prev_root: prev_root.clone(), // Clone before moving
            new_root: prev_root.clone(),  // Will be updated when operations are applied
            operations: Vec::new(),
        }
    }

    /// Add an insert operation
    pub fn insert(&mut self, key: String, value: String) {
        self.operations.push(StateOperation::Insert { key, value });
        self.update_root();
    }

    /// Add a delete operation
    pub fn delete(&mut self, key: String) {
        self.operations.push(StateOperation::Delete { key });
        self.update_root();
    }

    /// Update the root hash based on operations
    fn update_root(&mut self) {
        // In a real implementation, this would calculate a Merkle root
        // For now, we just create a simple hash of the operations count
        self.new_root = format!("root_{}", self.operations.len());
    }

    /// Convert from a map of modified values
    pub fn from_modifications(
        prev_root: String,
        modifications: HashMap<String, Option<String>>,
    ) -> Self {
        let mut diff = Self::new(prev_root);

        for (key, value_opt) in modifications {
            match value_opt {
                Some(value) => diff.insert(key, value),
                None => diff.delete(key),
            }
        }

        diff
    }
}

/// State observer trait for tracking changes
pub trait StateObserver: Send + Sync {
    /// Get the current state diff
    fn get_state_diff(&self) -> StateDiff;
}

/// Default implementation using copy-on-write
pub struct CopyOnWriteState {
    base_root: String,
    modifications: HashMap<String, Option<String>>,
}

impl CopyOnWriteState {
    /// Create a new copy-on-write state
    pub fn new(base_root: String) -> Self {
        Self {
            base_root,
            modifications: HashMap::new(),
        }
    }

    /// Read a value
    pub fn read(&self, key: &str) -> Option<String> {
        // In a real implementation, we'd check the modifications first,
        // then fall back to the base state
        self.modifications.get(key).cloned().unwrap_or(None)
    }

    /// Write a value
    pub fn write(&mut self, key: String, value: String) {
        self.modifications.insert(key, Some(value));
    }

    /// Delete a value
    pub fn delete(&mut self, key: String) {
        self.modifications.insert(key, None);
    }
}

impl StateObserver for CopyOnWriteState {
    fn get_state_diff(&self) -> StateDiff {
        StateDiff::from_modifications(self.base_root.clone(), self.modifications.clone())
    }
}
