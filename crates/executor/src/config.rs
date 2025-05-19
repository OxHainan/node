use mp_container::ContainerEnvironment;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::ExecutionEngineType;

/// Execution engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutorConfig {
    /// Number of worker threads for processing execution requests
    pub worker_threads: usize,
    /// Maximum concurrent execution requests
    pub max_concurrent_requests: usize,
    /// Enable detailed execution tracing
    pub enable_tracing: bool,

    /// Type of execution engine to use
    #[serde(skip)]
    pub engine_type: ExecutionEngineType,
    /// Container environment for container-based execution
    #[serde(skip)]
    pub container_environment: Option<Arc<dyn ContainerEnvironment>>,
    /// Compute environment for network-based execution (deprecated, use container_environment instead)
    #[serde(skip)]
    pub compute_environment: Option<Arc<dyn ContainerEnvironment>>,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            worker_threads: 4,
            max_concurrent_requests: 100,
            enable_tracing: false,
            engine_type: ExecutionEngineType::default(),
            container_environment: None,
            compute_environment: None,
        }
    }
}
