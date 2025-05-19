use anyhow::{anyhow, Result};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server, StatusCode};
use mp_sdk::{mpClient, TransactionBuilder, TransactionStatus, TransactionType};
use once_cell;
use primitive_types::H128;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::api_key_store::ApiKeyStore;

/// Configuration for the RESTful Gateway
#[derive(Debug, Clone, Deserialize)]
pub struct GatewayConfig {
    /// URL of the mp node
    pub node_url: String,
    /// Path to the API key database
    pub key_store_path: String,
    /// Bind address for the gateway
    pub gateway_bind_address: String,
    /// Bind address for the admin interface
    pub admin_bind_address: String,
    /// Transaction timeout in seconds
    pub tx_timeout: u64,
}

/// RESTful Gateway for mp
/// Translates HTTP requests into blockchain transactions
pub struct mpRestGateway {
    /// Client to interact with mp node
    node_client: mpClient,
    /// Store for API keys and their associated blockchain addresses
    api_key_store: Arc<ApiKeyStore>,
    /// Gateway configuration
    config: GatewayConfig,
}

impl mpRestGateway {
    /// Create a new gateway instance
    pub fn new(config: GatewayConfig, api_key_store: Arc<ApiKeyStore>) -> Self {
        let node_client = mpClient::new(&config.node_url);

        Self {
            node_client,
            api_key_store,
            config,
        }
    }

    /// Start the gateway HTTP server
    pub async fn start(&self) -> Result<()> {
        let addr: SocketAddr = self.config.gateway_bind_address.parse()?;
        let api_key_store = self.api_key_store.clone();
        let node_client = self.node_client.clone();
        let timeout = self.config.tx_timeout;

        let make_service = make_service_fn(move |_| {
            let api_key_store = api_key_store.clone();
            let node_client = node_client.clone();
            let timeout = timeout;

            async move {
                Ok::<_, hyper::Error>(service_fn(move |req| {
                    let api_key_store = api_key_store.clone();
                    let node_client = node_client.clone();
                    let timeout = timeout;

                    async move { handle_request(req, node_client, api_key_store, timeout).await }
                }))
            }
        });

        let server = Server::bind(&addr).serve(make_service);
        info!("mp RESTful Gateway listening on {}", addr);

        server.await.map_err(|e| anyhow!("Server error: {}", e))
    }
}

/// Handle incoming HTTP requests
async fn handle_request(
    req: Request<Body>,
    mut node_client: mpClient, // 添加 mut 关键字
    api_key_store: Arc<ApiKeyStore>,
    timeout: u64,
) -> Result<Response<Body>, hyper::Error> {
    // Extract API key from request
    let api_key = extract_api_key(&req);

    // Get blockchain address and nonce for this API key
    let (address, nonce) = match &api_key {
        // 使用引用而不是移动
        Some(key) => match api_key_store.get_address_and_nonce(key).await {
            // 使用引用
            Ok(info) => info,
            Err(_) => return Ok(unauthorized_response()),
        },
        None => return Ok(unauthorized_response()),
    };

    debug!(
        "Processing request with API key -> address: {}, nonce: {}",
        address, nonce
    );

    // Convert HTTP request to transaction payload
    let payload = match convert_request_to_payload(req).await {
        Ok(p) => p,
        Err(e) => {
            return Ok(internal_error_response(&format!(
                "Failed to process request: {}",
                e
            )))
        }
    };

    // Generate a unique ID for this request
    let request_id = H128::from(Uuid::new_v4().to_bytes_le()); // Generate H128 from Uuid

    // Create transaction with the unique request ID
    let tx_builder =
        TransactionBuilder::new(TransactionType::Request(request_id, "api".to_string()));

    // 添加有效载荷并构建事务
    let tx = match tx_builder.payload(&payload) {
        Ok(builder) => {
            // payload() 成功，继续添加 sender 并构建事务
            // 注意：sender() 返回 Self，build() 直接返回 Transaction
            builder.sender(&address).build()
        }
        Err(e) => {
            error!("Failed to set transaction payload: {}", e);
            return Ok(internal_error_response(&format!(
                "Failed to create transaction: {}",
                e
            )));
        }
    };

    // Submit transaction to mempool
    let tx_id = match node_client.submit_transaction(tx).await {
        Ok(id) => id,
        Err(e) => {
            return Ok(internal_error_response(&format!(
                "Transaction submission failed: {}",
                e
            )))
        }
    };

    info!("Submitted transaction: {} for API key", tx_id);

    // Increment nonce for the account
    if let Some(key) = &api_key {
        // 使用 if let 模式而不是 unwrap
        if let Err(e) = api_key_store.increment_nonce(key).await {
            error!("Failed to increment nonce: {}", e);
        }
    }

    // Wait for transaction confirmation
    let result = match node_client.wait_for_transaction(tx_id, timeout).await {
        Ok(status) => status,
        Err(e) => {
            return Ok(internal_error_response(&format!(
                "Transaction failed: {}",
                e
            )))
        }
    };

    info!("Transaction completed: {}", tx_id);

    // Convert transaction result to HTTP response
    Ok(transaction_result_to_response(result))
}

/// Extract API key from request
fn extract_api_key(req: &Request<Body>) -> Option<String> {
    // Try to get from Authorization header
    if let Some(auth) = req.headers().get("Authorization") {
        if let Ok(auth_str) = auth.to_str() {
            if auth_str.starts_with("Bearer ") {
                return Some(auth_str[7..].to_string());
            }
        }
    }

    // Try to get from X-API-Key header
    if let Some(key) = req.headers().get("X-API-Key") {
        if let Ok(key_str) = key.to_str() {
            return Some(key_str.to_string());
        }
    }

    // Try to get from query parameter
    if let Some(query) = req.uri().query() {
        for pair in query.split('&') {
            if let Some((key, value)) = pair.split_once('=') {
                if key == "api_key" {
                    return Some(value.to_string());
                }
            }
        }
    }

    None
}

/// Convert HTTP request to transaction payload
async fn convert_request_to_payload(req: Request<Body>) -> Result<serde_json::Value, hyper::Error> {
    let method = req.method().to_string();
    let uri = req.uri().to_string();
    let path = req.uri().path().to_string();
    let query = req.uri().query().map(|q| q.to_string());

    // Extract headers
    let headers_map = req
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or_default().to_string()))
        .collect::<HashMap<String, String>>();

    // Extract body
    let body_bytes = hyper::body::to_bytes(req.into_body()).await?;
    let body_str = String::from_utf8_lossy(&body_bytes);

    // Create payload JSON
    let payload = json!({
        "method": method,
        "uri": uri,
        "path": path,
        "query": query,
        "headers": headers_map,
        "body": body_str,
        "timestamp": chrono::Utc::now().timestamp(),
    });

    Ok(payload)
}

/// Convert transaction result to HTTP response
fn transaction_result_to_response(result: TransactionStatus) -> Response<Body> {
    match result {
        TransactionStatus::Confirmed(Some(output)) => {
            // Extract HTTP status, headers and body from output
            let status = output.get("status").and_then(|s| s.as_u64()).unwrap_or(200);

            // Create a longer-lived headers map with a let binding
            let headers_map = match output.get("headers") {
                Some(serde_json::Value::Object(map)) => map,
                _ => {
                    // Create an empty map that lives long enough
                    static EMPTY_MAP: once_cell::sync::Lazy<
                        serde_json::Map<String, serde_json::Value>,
                    > = once_cell::sync::Lazy::new(|| serde_json::Map::new());
                    &EMPTY_MAP
                }
            };

            let body = output.get("body").and_then(|b| b.as_str()).unwrap_or("");

            // Build response
            let mut response = Response::builder()
                .status(StatusCode::from_u16(status as u16).unwrap_or(StatusCode::OK));

            // Add headers
            for (key, value) in headers_map {
                if let Some(value_str) = value.as_str() {
                    response = response.header(key, value_str);
                }
            }

            response
                .body(Body::from(body.to_string()))
                .unwrap_or_default()
        }
        TransactionStatus::Confirmed(None) => Response::builder()
            .status(StatusCode::NO_CONTENT)
            .body(Body::empty())
            .unwrap_or_default(),
        TransactionStatus::Failed(error) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from(format!("Transaction failed: {}", error)))
            .unwrap_or_default(),
        _ => Response::builder()
            .status(StatusCode::GATEWAY_TIMEOUT)
            .body(Body::from("Transaction processing timed out"))
            .unwrap_or_default(),
    }
}

/// Create unauthorized response
fn unauthorized_response() -> Response<Body> {
    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .body(Body::from("Invalid or missing API key"))
        .unwrap_or_default()
}

/// Create internal error response
fn internal_error_response(message: &str) -> Response<Body> {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(Body::from(message.to_string()))
        .unwrap_or_default()
}
