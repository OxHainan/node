use anyhow::Result;
use async_trait::async_trait;
use http::{HeaderMap, HeaderValue};
use mp_common::types::TransactionType;
use mp_common::TransactionResponse;
use mp_poc::bls::SignedAggregate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ExecutionError;
use crate::metadata::ExecutionMetadata;
use crate::state::StateDiff;

/// Execution request payload
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExecutionRequest {
    /// Module identifier
    pub transaction_type: TransactionType,
    /// Input parameters in JSON format
    pub input: Vec<u8>,
    /// Transaction hash for tracing
    pub tx_hash: Uuid,
    /// HTTP method (GET, POST, etc)
    #[serde(skip)]
    pub method: http::Method,
    /// HTTP headers
    #[serde(skip)]
    pub header: HeaderMap<HeaderValue>,
}

/// Execution result structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExecutionResult {
    pub input: Vec<u8>,
    /// JSON output from the module
    pub output: TransactionResponse,
    /// State changes caused by execution
    pub state_diff: StateDiff,
    /// Blockchain-related metadata
    pub metadata: ExecutionMetadata,
    /// HTTP headers
    #[serde(skip)]
    pub headers: HeaderMap<HeaderValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResponse {
    pub result: ExecutionResult,
    // 接入poc
    pub signed_aggregate: SignedAggregate,
}

/// Core execution engine trait
#[async_trait]
pub trait ExecutionEngine: Send + Sync {
    /// Execute a module with given input
    async fn execute(
        &self,
        request: &mut ExecutionRequest,
    ) -> Result<ExecutionResult, ExecutionError>;

    /// Start the execution engine
    async fn start(&self, module_id: &Uuid) -> Result<()>;

    /// Stop the execution engine
    async fn stop(&self, module_id: &Uuid) -> Result<()>;
}
