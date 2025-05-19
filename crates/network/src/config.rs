use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Listen address
    pub listen_address: SocketAddr,

    /// Bootstrap nodes
    pub bootstrap_nodes: Vec<String>,
}
