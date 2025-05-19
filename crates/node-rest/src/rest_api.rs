use anyhow::{anyhow, Result};
use hyper::body::Bytes;
use hyper::header::HeaderValue;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, HeaderMap, Method, Request, Response, Server, StatusCode, Uri};
use mp_common::types::{Transaction, TransactionStatusWithProof, TransactionType};
use mp_executor::core::ExecutionRequest;
use mp_mempool::TransactionPool;
use serde::Deserialize;
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::api_key_store::ApiKeyStore;

/// Configuration for the integrated RESTful API
#[derive(Debug, Clone, Deserialize)]
pub struct RestApiConfig {
    /// Path to the API key database
    pub key_store_path: String,
    /// Bind address for the REST API
    pub rest_bind_address: String,
    /// Bind address for the admin interface
    pub admin_bind_address: String,
    /// Transaction timeout in seconds
    pub tx_timeout: u64,
}

/// Integrated RESTful API for mp Node
pub struct IntegratedRestApi {
    /// Local execution engine (no network calls)
    execution_request_sender: Sender<(
        ExecutionRequest,
        oneshot::Sender<TransactionStatusWithProof>,
    )>,
    /// Local transaction pool (no network calls)
    tx_pool: Arc<dyn TransactionPool + Send + Sync>,
    /// Store for API keys and their associated blockchain addresses
    api_key_store: Arc<ApiKeyStore>,
    /// REST API configuration
    config: RestApiConfig,
}

impl IntegratedRestApi {
    /// Create a new integrated REST API
    pub fn new(
        config: RestApiConfig,
        execution_request_sender: Sender<(
            ExecutionRequest,
            oneshot::Sender<TransactionStatusWithProof>,
        )>,
        tx_pool: Arc<dyn TransactionPool + Send + Sync>,
        api_key_store: Arc<ApiKeyStore>,
    ) -> Self {
        Self {
            execution_request_sender,
            tx_pool,
            api_key_store,
            config,
        }
    }

    /// Start the REST API HTTP server
    pub async fn start(&self) -> Result<()> {
        let addr: SocketAddr = self.config.rest_bind_address.parse()?;
        let api_key_store = self.api_key_store.clone();
        let tx_pool = self.tx_pool.clone();
        let execution_request_sender = self.execution_request_sender.clone();

        let make_service = make_service_fn(move |_| {
            let api_key_store = api_key_store.clone();
            let tx_pool = tx_pool.clone();
            let execution_request_sender = execution_request_sender.clone();

            async move {
                Ok::<_, hyper::Error>(service_fn(move |req| {
                    let api_key_store = api_key_store.clone();
                    let tx_pool = tx_pool.clone();
                    let execution_request_sender = execution_request_sender.clone();

                    async move {
                        handle_request(req, tx_pool, execution_request_sender, api_key_store).await
                    }
                }))
            }
        });

        let server = Server::bind(&addr).serve(make_service);
        info!("mp Integrated RESTful API listening on {}", addr);

        server.await.map_err(|e| anyhow!("Server error: {}", e))
    }
}

/// Handle incoming HTTP requests
async fn handle_request(
    mut req: Request<Body>,
    tx_pool: Arc<dyn TransactionPool + Send + Sync>,
    execution_sender: Sender<(
        ExecutionRequest,
        oneshot::Sender<TransactionStatusWithProof>,
    )>,
    api_key_store: Arc<ApiKeyStore>,
) -> Result<Response<Body>, hyper::Error> {
    let payload = match RequestToPayload::from_request(&mut req).await {
        Ok(p) => p,
        Err(e) => {
            error!("Failed to parse request payload: {}", e);
            return Ok(internal_error_response("Failed to parse request"));
        }
    };

    let Some(handle) = payload.path() else {
        return Ok(internal_error_response(&format!(
            "Failed to parse request path: {}",
            payload.path.path()
        )));
    };

    let address = if !handle.is_request() {
        // Extract API key from request
        let api_key = extract_api_key(&req);

        // Get blockchain address and nonce for this API key
        let (address, nonce) = match &api_key {
            Some(key) => match api_key_store.get_address_and_nonce(key).await {
                Ok(info) => info,
                Err(_) => return Ok(unauthorized_response()),
            },
            None => return Ok(unauthorized_response()),
        };

        debug!(
            "Processing request with API key -> address: {}, nonce: {}",
            address, nonce
        );

        // Increment nonce for the account
        if let Some(key) = &api_key {
            if let Err(e) = api_key_store.increment_nonce(key).await {
                error!("Failed to increment nonce: {}", e);
            }
        }

        Some(address)
    } else {
        None
    };
    println!("[REST] payload: {:?}", hex::encode(&payload.body.to_vec()));

    // Create local transaction (no network overhead)
    let tx_id = Uuid::new_v4();
    let tx = Transaction {
        id: tx_id,
        tx_type: handle.clone(),
        method: payload.method.clone(),
        header: payload.headers.clone(),
        payload: payload.body.to_vec(),
        sender: address,
        timestamp: chrono::Utc::now(),
        log_index: 0,
    };

    // Create a oneshot channel to receive results - this is the core part of the proactive notification system
    let (result_sender, result_receiver) = oneshot::channel();
    info!("Created new oneshot channel for transaction: {}", tx_id);

    // Submit transaction to local mempool
    match tx_pool.submit_transaction(tx.clone()).await {
        Ok(_) => info!("Submitted transaction: {} for API key", tx_id),
        Err(e) => {
            return Ok(internal_error_response(&format!(
                "Transaction submission failed: {}",
                e
            )))
        }
    };

    // Directly send execution request to execution engine
    info!(
        "Sending execution request directly: handler={:?}, tx_hash={}",
        handle, tx_id
    );

    let request = ExecutionRequest {
        transaction_type: handle,
        input: payload.body.to_vec(),
        tx_hash: tx_id,
        method: tx.method,
        header: tx.header,
    };

    if let Err(e) = execution_sender.send((request, result_sender)).await {
        error!("Failed to send execution request: {}", e);
        return Ok(internal_error_response("Failed to send execution request"));
    }
    info!("Successfully sent execution request for tx: {}", tx_id);

    let status_enum = result_receiver.await.unwrap();

    // Convert transaction result to HTTP response
    Ok(transaction_result_to_response(status_enum))
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

    None
}

pub struct RequestToPayload {
    pub method: Method,
    pub path: Uri,
    pub headers: HeaderMap<HeaderValue>,
    pub body: Bytes,
    pub query: Option<String>,
}

impl RequestToPayload {
    pub async fn from_request(req: &mut Request<Body>) -> Result<Self, hyper::Error> {
        Ok(Self {
            method: req.method().clone(),
            path: req.uri().clone(),
            headers: req.headers().clone(),
            body: hyper::body::to_bytes(req.body_mut()).await?,
            query: req.uri().query().map(|q| q.to_string()),
        })
    }

    pub fn path(&self) -> Option<TransactionType> {
        TransactionType::parse(self.path.path())
    }
}

/// Convert transaction result to HTTP response
fn transaction_result_to_response(result: TransactionStatusWithProof) -> Response<Body> {
    match result {
        TransactionStatusWithProof::Confirmed(output, status, headers, poc) => {
            // Check if output contains HTTP response fields
            let mut builder = Response::builder().status(status);
            if let Some(poc) = poc {
                builder = builder.header("X-PoC", serde_json::to_string(&poc).unwrap());
            }

            if let Some(headers) = headers {
                for (key, value) in headers {
                    if let Some(key) = key {
                        if key.as_str() == "content-length" {
                            continue;
                        }
                        builder = builder.header(key, value);
                    }
                }
            }

            builder
                .body(Body::from(output.to_string()))
                .unwrap_or_else(|_| {
                    Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::from("Error building response"))
                        .unwrap()
                })
        }
        TransactionStatusWithProof::Pending => Response::builder()
            .status(StatusCode::ACCEPTED)
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"status": "pending"}).to_string()))
            .unwrap(),
        TransactionStatusWithProof::Processing => Response::builder()
            .status(StatusCode::ACCEPTED)
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"status": "processing"}).to_string()))
            .unwrap(),
        TransactionStatusWithProof::Failed(reason, status, headers, poc) => {
            let mut builder = Response::builder().status(status);
            if let Some(poc) = poc {
                builder = builder.header("X-PoC", serde_json::to_string(&poc).unwrap());
            }

            if let Some(headers) = headers {
                for (key, value) in headers {
                    if let Some(key) = key {
                        if key.as_str() == "content-length" {
                            continue;
                        }
                        builder = builder.header(key, value);
                    }
                }
            }

            builder
                .body(Body::from(reason.to_string()))
                .unwrap_or_else(|e| {
                    error!("Error building response: {}", e);
                    Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::from("Error building response"))
                        .unwrap()
                })
        }
    }
}

/// Create unauthorized response
fn unauthorized_response() -> Response<Body> {
    let error = json!({"error": "Unauthorized"});

    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&error).unwrap()))
        .unwrap()
}

/// Create internal error response
fn internal_error_response(message: &str) -> Response<Body> {
    let error = json!({"error": message});

    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&error).unwrap()))
        .unwrap()
}
