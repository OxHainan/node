use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Execution metadata for blockchain integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMetadata {
    /// Transaction hash
    pub tx_hash: Uuid,
    /// Timestamp when execution completed
    pub executed_at: DateTime<Utc>,
    /// Gas used by execution
    pub gas_used: u64,
}

impl ExecutionMetadata {
    /// Create new execution metadata
    pub fn new(tx_hash: Uuid) -> Self {
        Self {
            tx_hash,
            executed_at: Utc::now(),
            gas_used: 0,
        }
    }

    /// Convert to JSON representation
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}
