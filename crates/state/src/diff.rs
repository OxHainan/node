use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Operation to be applied to the state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StateOperation {
    /// Insert or update a key with a value
    Insert {
        /// Key to insert or update
        key: String,
        /// Value to set
        value: String,
    },

    /// Delete a key
    Delete {
        /// Key to delete
        key: String,
    },
}

/// State difference representing changes to be applied
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StateDiff {
    /// Previous state root hash
    pub prev_root: String,

    /// New state root hash after changes
    pub new_root: String,

    /// Operations to apply to the state
    pub operations: Vec<StateOperation>,
}

impl StateDiff {
    /// Create a new state diff with the given previous root
    pub fn new(prev_root: String) -> Self {
        Self {
            prev_root: prev_root.clone(), // Clone before moving
            new_root: prev_root,          // Take ownership here
            operations: Vec::new(),
        }
    }

    /// Add an insert operation
    pub fn insert(&mut self, key: String, value: String) {
        self.operations.push(StateOperation::Insert { key, value });
    }

    /// Add a delete operation
    pub fn delete(&mut self, key: String) {
        self.operations.push(StateOperation::Delete { key });
    }

    /// Check if this diff is empty (no operations)
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    /// Get the number of operations in this diff
    pub fn len(&self) -> usize {
        self.operations.len()
    }
}

/// Extension trait for state storage to support diff-based updates
pub trait StateDiffStorage {
    /// Apply a state diff to the storage
    fn apply_diff(&self, diff: &StateDiff) -> Result<()>;

    /// Create a checkpoint of the current state
    /// Returns a StateDiff with the current state root but no operations
    fn create_checkpoint(&self) -> Result<StateDiff>;

    /// Apply multiple diffs in a batch
    fn batch_apply_diffs(&self, diffs: Vec<StateDiff>) -> Result<()> {
        for diff in diffs {
            self.apply_diff(&diff)?;
        }
        Ok(())
    }
}
