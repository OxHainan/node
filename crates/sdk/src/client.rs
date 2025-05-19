//! Client for interacting with a mp node

use crate::error::{Error, Result};
use crate::transaction::TransactionStatus;
use base64::Engine;
use mp_common::types::Transaction;
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::debug;
use uuid::Uuid;

/// JSON-RPC request
#[derive(Debug, Serialize)]
struct JsonRpcRequest<T> {
    jsonrpc: String,
    id: u64,
    method: String,
    params: T,
}

/// JSON-RPC response
#[derive(Debug, Deserialize)]
struct JsonRpcResponse<T> {
    jsonrpc: String,
    id: u64,
    result: Option<T>,
    error: Option<JsonRpcError>,
}

/// JSON-RPC error
#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

/// Client for interacting with a mp node
#[derive(Debug, Clone)]
pub struct mpClient {
    /// Node URL
    node_url: String,
    /// HTTP client
    http_client: HttpClient,
    /// Request ID counter
    request_id: u64,
}

impl mpClient {
    /// Create a new mp client
    pub fn new(node_url: impl Into<String>) -> Self {
        let http_client = HttpClient::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            node_url: node_url.into(),
            http_client,
            request_id: 1,
        }
    }

    /// Get the next request ID
    fn next_request_id(&mut self) -> u64 {
        let id = self.request_id;
        self.request_id += 1;
        id
    }

    /// Send a JSON-RPC request
    async fn send_request<P: Serialize + std::fmt::Debug, R: for<'de> Deserialize<'de>>(
        &mut self,
        method: &str,
        params: P,
    ) -> Result<R> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: self.next_request_id(),
            method: method.to_string(),
            params,
        };

        debug!("Sending request to {}: {:?}", self.node_url, request);

        let response = self
            .http_client
            .post(&self.node_url)
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::Http(e))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(Error::NodeCommunication(format!(
                "HTTP error {}: {}",
                status, error_text
            )));
        }

        let json_response: JsonRpcResponse<R> = response
            .json()
            .await
            .map_err(|e| Error::NodeCommunication(format!("Failed to parse response: {}", e)))?;

        if let Some(error) = json_response.error {
            return Err(Error::NodeCommunication(format!(
                "JSON-RPC error {}: {}",
                error.code, error.message
            )));
        }

        json_response
            .result
            .ok_or_else(|| Error::NodeCommunication("No result in response".to_string()))
    }

    /// Submit a transaction to the node
    pub async fn submit_transaction(&mut self, transaction: Transaction) -> Result<Uuid> {
        #[derive(Serialize, Debug)]
        struct SubmitParams {
            tx_type: String,
            payload: String,
            sender: Option<String>,
        }

        #[derive(Serialize, Debug)]
        struct SubmitTransactionRequest {
            params: SubmitParams,
        }

        let tx_type = format!("{:?}", transaction.tx_type);
        let payload = serde_json::to_string(&transaction.payload).map_err(|e| {
            Error::TransactionSubmission(format!("Failed to serialize payload: {}", e))
        })?;

        let params = SubmitParams {
            tx_type,
            payload,
            sender: transaction.sender,
        };

        let request = SubmitTransactionRequest { params };

        let result: serde_json::Value = self.send_request("submit_transaction", request).await?;

        let tx_id = result["tx_id"]
            .as_str()
            .ok_or_else(|| Error::TransactionSubmission("Missing tx_id in response".to_string()))?;

        let tx_id = Uuid::parse_str(tx_id)
            .map_err(|e| Error::TransactionSubmission(format!("Invalid tx_id: {}", e)))?;

        Ok(tx_id)
    }

    /// Get the status of a transaction
    pub async fn get_transaction_status(&mut self, tx_id: Uuid) -> Result<TransactionStatus> {
        #[derive(Serialize, Debug)]
        struct TxStatusParams {
            tx_id: String,
        }

        let params = TxStatusParams {
            tx_id: tx_id.to_string(),
        };

        let result: serde_json::Value = self.send_request("getTransactionStatus", params).await?;

        let status = result["status"]
            .as_str()
            .ok_or_else(|| Error::TransactionNotFound("Missing status in response".to_string()))?;

        match status {
            "pending" => Ok(TransactionStatus::Pending),
            "processing" => Ok(TransactionStatus::Processing),
            "confirmed" | "success" => {
                // Extract the execution result if available
                // IMPORTANT: Do not add an additional layer of nesting/wrapping
                // The result field already contains the actual execution result from the handler
                // Using .get() here would create an extra nesting level
                let execution_result = if result.get("result").is_some() {
                    result["result"].clone()
                } else {
                    // No result field found - this is unusual
                    debug!(
                        "Transaction status response missing 'result' field, response: {:?}",
                        result
                    );
                    serde_json::Value::Null
                };

                // Only wrap in Option if we have a non-null value
                let opt_result = if execution_result.is_null() {
                    None
                } else {
                    Some(execution_result)
                };

                Ok(TransactionStatus::Confirmed(opt_result))
            }
            "failed" => {
                let reason = result["reason"]
                    .as_str()
                    .unwrap_or("Unknown reason")
                    .to_string();
                Ok(TransactionStatus::Failed(reason))
            }
            "not_found" => Ok(TransactionStatus::NotFound),
            _ => Err(Error::InvalidTransactionData(format!(
                "Unknown status: {}",
                status
            ))),
        }
    }

    /// Wait for a transaction to be confirmed
    pub async fn wait_for_transaction(
        &mut self,
        tx_id: Uuid,
        timeout_secs: u64,
    ) -> Result<TransactionStatus> {
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(timeout_secs);
        let mut attempt_count = 0;

        loop {
            attempt_count += 1;
            let status = self.get_transaction_status(tx_id).await?;
            debug!(
                "[Transaction {}] Status check #{}: {:?}",
                tx_id, attempt_count, status
            );

            match status {
                TransactionStatus::Confirmed(result) => {
                    debug!(
                        "[Transaction {}] Confirmed with result: {:?}",
                        tx_id, result
                    );
                    return Ok(TransactionStatus::Confirmed(result));
                }
                TransactionStatus::Failed(reason) => {
                    debug!("[Transaction {}] Failed with reason: {}", tx_id, reason);
                    return Ok(TransactionStatus::Failed(reason));
                }
                TransactionStatus::NotFound => {
                    if start.elapsed() > timeout {
                        return Err(Error::TransactionNotFound(format!(
                            "Transaction {} not found after {} seconds (checked {} times)",
                            tx_id, timeout_secs, attempt_count
                        )));
                    }
                    debug!("[Transaction {}] Not found yet, waiting...", tx_id);
                }
                _ => {
                    if start.elapsed() > timeout {
                        return Err(Error::TransactionNotFound(format!(
                            "Transaction {} not confirmed after {} seconds (checked {} times)",
                            tx_id, timeout_secs, attempt_count
                        )));
                    }
                    debug!("[Transaction {}] Status: {:?}, waiting...", tx_id, status);
                }
            }

            // Use a shorter polling interval initially, then gradually increase it
            let wait_time = if attempt_count < 5 {
                Duration::from_millis(100)
            } else if attempt_count < 10 {
                Duration::from_millis(250)
            } else {
                Duration::from_millis(500)
            };

            tokio::time::sleep(wait_time).await;
        }
    }

    /// Send an API request to a smart contract
    pub async fn api_request<T: Serialize>(
        &mut self,
        contract_id: &str,
        endpoint: &str,
        payload: &T,
    ) -> Result<serde_json::Value> {
        #[derive(Serialize, Debug)]
        struct ApiRequestParams {
            contract_id: String,
            endpoint: String,
            payload: String,
        }

        #[derive(Serialize, Debug)]
        struct ApiRequestWrapper {
            params: ApiRequestParams,
        }

        let payload_json = serde_json::to_vec(payload)?;
        let payload_base64 = base64::engine::general_purpose::STANDARD.encode(payload_json);

        let params = ApiRequestParams {
            contract_id: contract_id.to_string(),
            endpoint: endpoint.to_string(),
            payload: payload_base64,
        };

        let request = ApiRequestWrapper { params };

        self.send_request("api_request", request).await
    }

    /// Get node information
    pub async fn get_node_info(&mut self) -> Result<serde_json::Value> {
        self.send_request::<(), _>("get_node_info", ()).await
    }
}
