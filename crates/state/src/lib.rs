pub mod config;
pub mod db;
pub mod diff;

use anyhow::Result;
use mp_common::types::Transaction;
use std::sync::Arc;

use crate::diff::StateDiffStorage;

/// State storage interface
pub trait StateStorage: Send + Sync + StateDiffStorage {
    /// Start the state storage
    fn start(&self) -> Result<()>;

    /// Stop the state storage
    fn stop(&self) -> Result<()>;

    /// Apply a transaction to the state
    fn apply_transaction(&self, transaction: Transaction) -> Result<()>;

    /// Get the current state root hash
    fn get_state_root(&self) -> Result<String>;

    /// Clone this state storage instance
    fn clone(&self) -> Arc<dyn StateStorage>;
}

/// Create a new state storage based on the configuration
pub fn create_state_storage(
    config: config::StateConfig,
) -> Result<Arc<dyn StateStorage + 'static>> {
    match config.db_type.as_str() {
        "sqlite" => {
            let storage = db::sqlite::SqliteStateStorage::new(config)?;
            Ok(Arc::new(storage))
        }
        _ => Err(anyhow::anyhow!(
            "Unsupported database type: {}",
            config.db_type
        )),
    }
}
