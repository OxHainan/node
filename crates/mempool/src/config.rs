use serde::{Deserialize, Serialize};

/// Transaction pool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MempoolConfig {
    /// Maximum number of transactions in the pool
    pub max_transactions: usize,

    /// API server address
    pub api_address: Option<String>,

    /// Maximum transaction size in bytes
    pub max_tx_size: usize,

    /// Transaction timeout in seconds
    pub tx_timeout: u64,
}
