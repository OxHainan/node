pub mod bridge;
// pub mod compute; // 保留旧模块，稍后将被弃用
pub mod config;
pub mod container; // 新的容器模块
pub mod core;
pub mod error;
pub mod metadata;
pub mod state;

use anyhow::Result;
// Remove unused import: use mp_container::ContainerEnvironment;
use crate::container::ContainerExecutionEngine;
use crate::core::ExecutionEngine;
pub use mp_container::utils;
use std::sync::Arc;
use tracing::{info, warn};

/// Type of execution engine to use
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionEngineType {
    /// Local execution engine (in-process)
    Local,
    /// Network-based execution engine (Docker, Kubernetes, etc.)
    Network,
}

impl Default for ExecutionEngineType {
    fn default() -> Self {
        ExecutionEngineType::Local
    }
}

/// Create a new execution engine based on the configuration
pub async fn create_execution_engine(
    config: config::ExecutorConfig,
) -> Result<Arc<dyn ExecutionEngine>> {
    match config.engine_type {
        ExecutionEngineType::Network => {
            if let Some(container_env) = &config.container_environment {
                // Create a container-based execution engine
                info!("Creating container-based execution engine");
                let engine = ContainerExecutionEngine::new(config.clone(), container_env.clone())?;

                Ok(Arc::new(engine))
            } else {
                // No container environment provided, fall back to local execution
                warn!("No container environment provided for network execution, falling back to local execution");
                // let engine = LocalExecutionEngine::new(config)?;
                // Ok(Arc::new(engine))
                todo!()
            }
        }
        ExecutionEngineType::Local => {
            // Create a local execution engine
            info!("Creating local execution engine");
            // let engine = LocalExecutionEngine::new(config)?;
            // Ok(Arc::new(engine))
            todo!()
        }
    }
}
