use anyhow::Result;
use base64;
use jsonrpsee::core::{async_trait, Error as JsonRpcError};
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::server::RpcModule;
use mp_common::types::{ApiRequestPayload, TransactionStatus, TransactionStatusWithProof, TransactionType};
use mp_common::utils::create_transaction;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::TransactionPool;

/// JSON-RPC API for the transaction pool
#[rpc(server)]
pub trait PoolApi {
    /// Submit a Web2 API request
    #[method(name = "submitApiRequest")]
    async fn submit_api_request(&self, request: ApiRequest) -> Result<ApiResponse, JsonRpcError>;

    /// Get the status of a transaction
    #[method(name = "getTransactionStatus")]
    async fn get_transaction_status(
        &self,
        tx_id: Uuid,
    ) -> Result<TransactionStatusWithProof, JsonRpcError>;

    /// Get node information
    #[method(name = "get_node_info")]
    async fn get_node_info(&self) -> Result<serde_json::Value, JsonRpcError>;

    /// Submit a transaction
    #[method(name = "submit_transaction")]
    async fn submit_transaction(
        &self,
        params: SubmitTransactionParams,
    ) -> Result<SubmitTransactionResponse, JsonRpcError>;

    /// Send an API request
    #[method(name = "api_request")]
    async fn api_request(
        &self,
        params: ApiRequestParams,
    ) -> Result<serde_json::Value, JsonRpcError>;
}

/// API request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiRequest {
    /// HTTP method
    pub method: String,
    /// API path
    pub path: String,
    /// Request headers
    pub headers: Vec<(String, String)>,
    /// Request body as base64-encoded string
    pub body: String,
}

/// API response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse {
    /// Transaction ID
    pub tx_id: String,
    /// Transaction status
    pub status: String,
}

/// Submit transaction parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitTransactionParams {
    /// Transaction type
    pub tx_type: TransactionType,
    /// Payload as base64-encoded string
    pub payload: String,
    /// Sender
    pub sender: Option<String>,
}

/// Submit transaction response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitTransactionResponse {
    /// Transaction ID
    pub tx_id: String,
}

/// API request parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiRequestParams {
    /// Contract ID
    pub contract_id: String,
    /// Endpoint
    pub endpoint: String,
    /// HTTP method, defaults to POST if not specified
    pub method: Option<String>,
    /// Payload as base64-encoded string
    pub payload: String,
}

// /// Transaction status
// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub struct TransactionStatus {
//     /// Transaction ID
//     pub tx_id: String,
//     /// Transaction status
//     pub status: String,
//     /// Transaction result (if available)
//     pub result: Option<serde_json::Value>,
//     /// Error message (if any)
//     pub error: Option<String>,
// }

/// Transaction status enum for internal use
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionStatusEnum {
    /// Transaction is pending
    Pending,
    /// Transaction is being processed
    Processing,
    /// Transaction has been confirmed
    Confirmed(serde_json::Value),
    /// Transaction has failed
    Failed(serde_json::Value),
}

/// API error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    /// Error code
    pub code: i32,
    /// Error message
    pub message: String,
}

impl From<JsonRpcError> for ApiError {
    fn from(err: JsonRpcError) -> Self {
        ApiError {
            code: -32000,
            message: format!("JSON-RPC error: {}", err),
        }
    }
}

/// API server
pub struct ApiServer {
    pool: Arc<dyn TransactionPool>,
    address: SocketAddr,
}

impl ApiServer {
    /// Create a new API server
    pub fn new(pool: Arc<dyn TransactionPool>, address: SocketAddr) -> Self {
        Self { pool, address }
    }

    /// Start the API server
    pub async fn start(&self) -> Result<()> {
        info!("Starting API server on {}", self.address);

        let server = jsonrpsee::server::ServerBuilder::default()
            .build(self.address)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to build RPC server: {}", e))?;

        let pool = self.pool.clone();
        let rpc = RpcServerImpl { pool };

        let mut module = RpcModule::new(());
        module
            .merge(rpc.into_rpc())
            .map_err(|e| anyhow::anyhow!("Failed to merge RPC module: {}", e))?;

        let handle = server.start(module)?;

        info!("JSON-RPC server started successfully on {}", self.address);

        // Keep the server running
        tokio::spawn(async move {
            handle.stopped().await;
            info!("JSON-RPC server stopped");
        });

        Ok(())
    }
}

/// RPC server implementation
struct RpcServerImpl {
    pool: Arc<dyn TransactionPool>,
}

#[async_trait]
impl PoolApiServer for RpcServerImpl {
    async fn submit_api_request(&self, request: ApiRequest) -> Result<ApiResponse, JsonRpcError> {
        info!("Received API request: {} {}", request.method, request.path);

        // Decode base64 body
        let body = match base64::decode(&request.body) {
            Ok(b) => b,
            Err(e) => {
                error!("Failed to decode base64 body: {}", e);
                return Err(JsonRpcError::Custom(format!("Invalid base64 body: {}", e)));
            }
        };

        info!("Decoded request body, size: {} bytes", body.len());

        // Create API request payload
        let payload = ApiRequestPayload {
            method: request.method.clone(),
            headers: request.headers.clone(),
            body,
        };

        // Serialize payload
        let payload_bytes = match serde_json::to_vec(&payload) {
            Ok(b) => b,
            Err(e) => {
                error!("Failed to serialize payload: {}", e);
                return Err(JsonRpcError::Custom(format!(
                    "Failed to serialize payload: {}",
                    e
                )));
            }
        };

        info!("Created API payload, size: {} bytes", payload_bytes.len());

        // Create transaction
        let transaction = create_transaction(TransactionType::Request(request.path.clone()), None, payload_bytes, None);

        info!("Created transaction with ID: {}", transaction.id);

        // Submit transaction to pool
        let response = match self.pool.submit_transaction(transaction.clone()).await {
            Ok(resp) => resp,
            Err(e) => {
                error!("Failed to submit transaction: {}", e);
                return Err(JsonRpcError::Custom(format!(
                    "Failed to submit transaction: {}",
                    e
                )));
            }
        };

        if response.status == TransactionStatus::Error {
            let error_msg = response
                .result
                .map(|r| r.to_string())
                .unwrap_or_else(|| "Unknown error".to_string());
            error!("Transaction rejected: {}", error_msg);
            return Err(JsonRpcError::Custom(error_msg));
        }

        info!(
            "Successfully submitted transaction to pool: {}",
            transaction.id
        );

        info!("Returning API response for transaction: {}", transaction.id);

        Ok(ApiResponse {
            tx_id: transaction.id.to_string(),
            status: "pending".to_string(),
        })
    }

    async fn get_transaction_status(
        &self,
        tx_id: Uuid,
    ) -> Result<TransactionStatusWithProof, JsonRpcError> {
        info!(
            "API - Received transaction status request for ID: {}",
            tx_id
        );

        // Get transaction status enum
        let tx_status = self
            .pool
            .get_transaction_status(&tx_id)
            .await
            .map_err(|e| {
                JsonRpcError::Custom(format!("Failed to get transaction status: {}", e))
            })?;

        Ok(tx_status)
    }

    async fn get_node_info(&self) -> Result<serde_json::Value, JsonRpcError> {
        info!("Received node info request");

        // Return basic node information
        let node_info = serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "name": "mp Node",
            "status": "running",
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });

        info!("Returning node info");

        Ok(node_info)
    }

    async fn submit_transaction(
        &self,
        params: SubmitTransactionParams,
    ) -> Result<SubmitTransactionResponse, JsonRpcError> {
        info!("Received submit_transaction request");

        // Parse transaction type
        let tx_type = params.tx_type;

        // Decode base64 payload
        let payload = match base64::decode(&params.payload) {
            Ok(b) => b,
            Err(e) => {
                error!("Failed to decode base64 payload: {}", e);
                return Err(JsonRpcError::Custom(format!(
                    "Invalid base64 payload: {}",
                    e
                )));
            }
        };

        // Create transaction
        let transaction = create_transaction(tx_type, None, payload, params.sender);

        info!("Created transaction with ID: {}", transaction.id);

        // Submit transaction to pool
        match self.pool.submit_transaction(transaction.clone()).await {
            Ok(_) => {
                info!(
                    "Successfully submitted transaction to pool: {}",
                    transaction.id
                );
                Ok(SubmitTransactionResponse {
                    tx_id: transaction.id.to_string(),
                })
            }
            Err(e) => {
                error!("Failed to submit transaction: {}", e);
                Err(JsonRpcError::Custom(format!(
                    "Failed to submit transaction: {}",
                    e
                )))
            }
        }
    }

    async fn api_request(
        &self,
        params: ApiRequestParams,
    ) -> Result<serde_json::Value, JsonRpcError> {
        info!(
            "Received api_request: contract={}, endpoint={}, method={}",
            params.contract_id,
            params.endpoint,
            params.method.as_deref().unwrap_or("POST")
        );

        // Decode base64 payload
        let payload_bytes = match base64::decode(&params.payload) {
            Ok(b) => b,
            Err(e) => {
                error!("Failed to decode base64 payload: {}", e);
                return Err(JsonRpcError::Custom(format!(
                    "Invalid base64 payload: {}",
                    e
                )));
            }
        };

        // Create API request payload
        let api_payload = ApiRequestPayload {
            method: params.method.clone().unwrap_or_else(|| "POST".to_string()),
            headers: vec![("Content-Type".to_string(), "application/json".to_string())],
            body: payload_bytes,
        };

        // Serialize payload
        let payload = match serde_json::to_vec(&api_payload) {
            Ok(b) => b,
            Err(e) => {
                error!("Failed to serialize API payload: {}", e);
                return Err(JsonRpcError::Custom(format!(
                    "Failed to serialize API payload: {}",
                    e
                )));
            }
        };

        // Create transaction
        let transaction = create_transaction(
            TransactionType::Request(format!(
                "/contracts/{}{}",
                params.contract_id, params.endpoint
            )),
            None,
            payload,
            None,
        );

        info!(
            "Created API request transaction with ID: {}",
            transaction.id
        );

        // Submit transaction to pool
        match self.pool.submit_transaction(transaction.clone()).await {
            Ok(_response) => {
                info!("API request transaction submitted: {}", transaction.id);

                // In a real implementation, we would wait for the transaction to be executed
                // and return the actual result. For now, we'll return a placeholder.
                Ok(serde_json::json!({
                    "tx_id": transaction.id.to_string(),
                    "status": "pending",
                    "message": "API request submitted successfully"
                }))
            }
            Err(e) => {
                error!("Failed to submit API request transaction: {}", e);
                Err(JsonRpcError::Custom(format!(
                    "Failed to submit API request: {}",
                    e
                )))
            }
        }
    }
}

// impl From<TransactionStatusEnum> for TransactionStatus {
//     fn from(status_enum: TransactionStatusEnum) -> Self {
//         match status_enum {
//             TransactionStatusEnum::Pending => TransactionStatus {
//                 tx_id: "".to_string(), // Will be filled in by caller
//                 status: "pending".to_string(),
//                 result: None,
//                 error: None,
//             },
//             TransactionStatusEnum::Processing => TransactionStatus {
//                 tx_id: "".to_string(), // Will be filled in by caller
//                 status: "processing".to_string(),
//                 result: None,
//                 error: None,
//             },
//             TransactionStatusEnum::Confirmed(result) => TransactionStatus {
//                 tx_id: "".to_string(), // Will be filled in by caller
//                 status: "success".to_string(),
//                 result: Some(result),
//                 error: None,
//             },
//             TransactionStatusEnum::Failed(result) => TransactionStatus {
//                 tx_id: "".to_string(), // Will be filled in by caller
//                 status: "failed".to_string(),
//                 result: Some(result),
//                 error: Some("Transaction execution failed".to_string()),
//             },
//         }
//     }
// }
