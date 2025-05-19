use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Consensus configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusConfig {
    /// Type of consensus engine to use
    pub engine_type: String,

    /// Node ID in the consensus network
    pub node_id: u64,

    /// Addresses of all nodes in the consensus network
    pub nodes: Vec<NodeInfo>,

    /// Raft-specific configuration
    pub raft: Option<RaftConfig>,
}

/// Information about a node in the consensus network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    /// Node ID
    pub id: u64,

    /// Node address
    pub address: SocketAddr,
}

/// Raft-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaftConfig {
    /// Heartbeat interval in milliseconds
    pub heartbeat_interval: u64,

    /// Minimum election timeout in milliseconds
    pub election_timeout_min: u64,

    /// Maximum election timeout in milliseconds
    pub election_timeout_max: u64,

    /// Snapshot interval in number of log entries
    pub snapshot_interval: u64,

    /// Path to store Raft logs
    pub log_path: String,
}
