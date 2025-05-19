use anyhow; // Remove anyhow::anyhow import
use async_trait::async_trait;
use mp_common::types::Transaction;
use mp_common::utils::h128_to_uuid;
use mp_common::H128;
use mp_container::ContainerEnvironment;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::config::ExecutorConfig;
use crate::core::{ExecutionEngine, ExecutionRequest, ExecutionResult};
use crate::error::ExecutionError;
use crate::metadata::ExecutionMetadata;
use crate::state::StateDiff;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub domain: String,
    pub agent_name: String,
    pub description: String,
    pub tags: Vec<String>,
}

/// Container-based execution engine implementation
///
/// This execution engine uses a container environment (such as Docker)
/// to execute transactions in isolated containers. It manages the lifecycle of
/// containers and handles communication with them.
pub struct ContainerExecutionEngine {
    config: ExecutorConfig,
    container_env: Arc<dyn ContainerEnvironment>,
    containers: Mutex<HashMap<H128, ContainerInfo>>,
}

impl ContainerExecutionEngine {
    /// Create a new container-based execution engine
    pub fn new(
        config: ExecutorConfig,
        container_env: Arc<dyn ContainerEnvironment>,
    ) -> Result<Self, anyhow::Error> {
        Ok(Self {
            config,
            container_env,
            containers: Mutex::new(HashMap::new()),
        })
    }

    /// Create an API request transaction for execution
    fn create_api_request(
        &self,
        request: &ExecutionRequest,
    ) -> Result<Transaction, ExecutionError> {
        // Create transaction
        let transaction = mp_common::utils::create_transaction(
            request.transaction_type.clone(),
            request.input.clone(),
            Some(Uuid::new_v4().to_string()),
            request.method.clone(),
            request.header.clone(),
        );

        Ok(transaction)
    }
}

#[async_trait]
impl ExecutionEngine for ContainerExecutionEngine {
    async fn execute(
        &self,
        request: &mut ExecutionRequest,
    ) -> Result<ExecutionResult, ExecutionError> {
        // Get or create a container for this module
        info!(
            "ContainerExecutionEngine: Starting execution for tx {} in type {:?}",
            request.tx_hash, request.transaction_type
        );

        // Create an API request transaction
        let api_request = self.create_api_request(&request)?;

        // Execute the transaction in the container environment
        info!(
            "Executing transaction in container environment: {:?}",
            api_request.id
        );

        match self.container_env.execute_transaction(api_request).await {
            Ok(api_response) => {
                info!("ContainerExecutionEngine: Successfully executed transaction in container");

                // Parse the API response
                match serde_json::from_slice(&api_response.payload) {
                    Ok(output) => {
                        info!("ContainerExecutionEngine: Successfully parsed API response");
                        // For now, we just create an empty state diff and basic metadata
                        // In a real implementation, we would track state changes during execution
                        let state_diff = StateDiff::default();
                        let metadata = ExecutionMetadata {
                            tx_hash: request.tx_hash,
                            executed_at: chrono::Utc::now(),
                            gas_used: 1000, // Simulated gas usage
                        };

                        // Create the execution result
                        let result = ExecutionResult {
                            input: request.input.clone(),
                            output,
                            state_diff,
                            metadata,
                            headers: api_response.header.clone(),
                        };

                        info!(
                            "EXECUTOR - Returning execution result with output: {}",
                            serde_json::to_string_pretty(&result.output).unwrap_or_default()
                        );

                        Ok(result)
                    }
                    Err(e) => {
                        error!(
                            "ContainerExecutionEngine: Failed to parse API response: {}",
                            e
                        );
                        Err(ExecutionError::InternalError(e.to_string()))
                    }
                }
            }
            Err(e) => {
                error!(
                    "ContainerExecutionEngine: Failed to execute transaction in container: {}",
                    e
                );
                Err(ExecutionError::ExecutionError(format!(
                    "Failed to execute transaction: {}",
                    e
                )))
            }
        }
    }

    async fn start(&self, module_id: &Uuid) -> Result<(), anyhow::Error> {
        info!(
            "Starting container execution engine for module {}",
            module_id
        );

        // Start the container environment
        self.container_env
            .start_container(module_id)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to start container environment: {}", e))?;

        Ok(())
    }

    async fn stop(&self, module_id: &Uuid) -> Result<(), anyhow::Error> {
        info!(
            "Stopping container execution engine for module {}",
            module_id
        );

        // Stop all containers
        let containers = self.containers.lock().await;
        for (module_id, _) in containers.iter() {
            info!("Stopping container for module {}", module_id);
            if let Err(e) = self
                .container_env
                .stop_container(&h128_to_uuid(module_id))
                .await
            {
                warn!("Failed to stop container for module {}: {}", module_id, e);
            }
        }

        // Stop the container environment
        self.container_env
            .stop_container(module_id)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to stop container environment: {}", e))?;

        Ok(())
    }
}
