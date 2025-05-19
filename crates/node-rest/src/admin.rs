use anyhow::{anyhow, Result};
use dstack::{TappdClientT, TdxQuoteResponse, WorkerInfo};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use mp_poc::PublicKey;
use primitive_types::H384;
use serde::{Deserialize, Serialize};
use serde_human_bytes as hex_bytes;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info};

use crate::api_key_store::ApiKeyStore;

pub struct AdminInterface {
    /// API key store
    api_key_store: Arc<ApiKeyStore>,

    /// App environment
    app_env: Arc<Mutex<dyn TappdClientT>>,

    poc_quote: PoCQuote,
}

/// Request for generating an API key
#[derive(Debug, Deserialize)]
struct GenerateKeyRequest {
    /// Name of the API key (optional)
    name: Option<String>,
    /// Blockchain address to associate with the API key
    address: String,
}

/// Response for the generate key operation
#[derive(Debug, Serialize)]
struct GenerateKeyResponse {
    /// Generated API key
    api_key: String,
    /// Address associated with the API key
    address: String,
}

/// Response for the list keys operation
#[derive(Debug, Serialize)]
struct ListKeysResponse {
    /// API keys and their associated information
    keys: Vec<KeyInfo>,
}

/// API key information
#[derive(Debug, Serialize)]
struct KeyInfo {
    /// API key
    api_key: String,
    /// Name of the API key (optional)
    name: Option<String>,
    /// Blockchain address associated with the API key
    address: String,
    /// Current nonce for this address
    nonce: u64,
    /// Time when the API key was created
    created_at: String,
}

impl AdminInterface {
    /// Create a new admin interface
    pub fn new(
        api_key_store: Arc<ApiKeyStore>,
        app_env: Arc<Mutex<dyn TappdClientT>>,
        poc_quote: PoCQuote,
    ) -> Self {
        Self {
            api_key_store,
            app_env,
            poc_quote,
        }
    }

    /// Start the admin interface HTTP server
    pub async fn start(&self, bind_address: &str) -> Result<()> {
        let addr: SocketAddr = bind_address.parse()?;
        let api_key_store = self.api_key_store.clone();
        let app_env = self.app_env.clone();

        let make_service = make_service_fn(move |_| {
            let api_key_store = api_key_store.clone();
            let app_env = app_env.clone();
            let poc_quote = self.poc_quote.clone();

            async move {
                Ok::<_, hyper::Error>(service_fn(move |req| {
                    let api_key_store = api_key_store.clone();
                    let app_env = app_env.clone();
                    let poc_quote = poc_quote.clone();

                    async move { handle_admin_request(req, api_key_store, app_env, poc_quote).await }
                }))
            }
        });

        let server = Server::bind(&addr).serve(make_service);
        info!("Admin interface listening on {}", addr);

        server
            .await
            .map_err(|e| anyhow!("Admin server error: {}", e))
    }
}

/// Handle incoming admin HTTP requests
async fn handle_admin_request(
    req: Request<Body>,
    api_key_store: Arc<ApiKeyStore>,
    app_env: Arc<Mutex<dyn TappdClientT>>,
    poc_quote: PoCQuote,
) -> Result<Response<Body>, hyper::Error> {
    match (req.method(), req.uri().path()) {
        // Generate API key
        (&Method::POST, "/api-keys") => {
            // Parse request body
            let body_bytes = hyper::body::to_bytes(req.into_body()).await?;
            let generate_req: GenerateKeyRequest = match serde_json::from_slice(&body_bytes) {
                Ok(req) => req,
                Err(_) => return Ok(bad_request_response("Invalid request format")),
            };

            // Generate API key
            match api_key_store
                .generate_key(generate_req.name, &generate_req.address)
                .await
            {
                Ok(api_key) => {
                    let response = GenerateKeyResponse {
                        api_key,
                        address: generate_req.address,
                    };

                    let json = serde_json::to_string(&response).unwrap();

                    Ok(Response::builder()
                        .status(StatusCode::CREATED)
                        .header("Content-Type", "application/json")
                        .body(Body::from(json))
                        .unwrap())
                }
                Err(e) => {
                    error!("Failed to generate API key: {}", e);
                    Ok(internal_error_response("Failed to generate API key"))
                }
            }
        }

        // Revoke API key
        (&Method::DELETE, path) if path.starts_with("/api-keys/") => {
            let api_key = path.trim_start_matches("/api-keys/");

            // Revoke API key
            match api_key_store.revoke_key(api_key).await {
                Ok(_) => Ok(Response::builder()
                    .status(StatusCode::NO_CONTENT)
                    .body(Body::empty())
                    .unwrap()),
                Err(e) => {
                    error!("Failed to revoke API key: {}", e);
                    Ok(not_found_response("API key not found"))
                }
            }
        }

        // List API keys
        (&Method::GET, "/api-keys") => {
            // Get all API keys
            match api_key_store.get_all_keys().await {
                Ok(keys) => {
                    let key_info: Vec<KeyInfo> = keys
                        .iter()
                        .map(|(api_key, info)| KeyInfo {
                            api_key: api_key.clone(),
                            name: info.name.clone(),
                            address: info.address.clone(),
                            nonce: info.nonce,
                            created_at: info.created_at.to_rfc3339(),
                        })
                        .collect();

                    let response = ListKeysResponse { keys: key_info };

                    let json = serde_json::to_string(&response).unwrap();

                    Ok(Response::builder()
                        .status(StatusCode::OK)
                        .header("Content-Type", "application/json")
                        .body(Body::from(json))
                        .unwrap())
                }
                Err(e) => {
                    error!("Failed to list API keys: {}", e);
                    Ok(internal_error_response("Failed to list API keys"))
                }
            }
        }

        (&Method::GET, "/poc-quote") => {
            let json = serde_json::to_string(&poc_quote).unwrap();
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(Body::from(json))
                .unwrap())
        }

        (&Method::GET, "/node-info") => {
            let info = match app_env.lock().await.info().await {
                Ok(info) => info,
                Err(_) => mock_worker_info(),
            };

            let json = serde_json::to_string(&info).unwrap();
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(Body::from(json))
                .unwrap())
        }

        // Health check
        (&Method::GET, "/health") => Ok(Response::new(Body::from("Admin interface is healthy"))),

        // Not found
        _ => Ok(not_found_response("Endpoint not found")),
    }
}

/// Create a bad request response
fn bad_request_response(message: &str) -> Response<Body> {
    let error = serde_json::json!({
        "error": message,
    });

    Response::builder()
        .status(StatusCode::BAD_REQUEST)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&error).unwrap()))
        .unwrap()
}

/// Create a not found response
fn not_found_response(message: &str) -> Response<Body> {
    let error = serde_json::json!({
        "error": message,
    });

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&error).unwrap()))
        .unwrap()
}

/// Create an internal error response
fn internal_error_response(message: &str) -> Response<Body> {
    let error = serde_json::json!({
        "error": message,
    });

    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&error).unwrap()))
        .unwrap()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoCQuote {
    /// Quote
    #[serde(with = "hex_bytes")]
    quote: Vec<u8>,
    /// Event log
    event_log: String,
    /// Hash algorithm
    hash_algorithm: String,
    /// Prefix
    prefix: String,
    /// Aggregate public key
    aggregate_public_key: H384,
}

impl PoCQuote {
    pub fn new(quote: TdxQuoteResponse, aggregate_public_key: PublicKey) -> Self {
        Self {
            quote: quote.quote,
            event_log: quote.event_log,
            hash_algorithm: quote.hash_algorithm,
            prefix: quote.prefix,
            aggregate_public_key: H384::from_slice(&aggregate_public_key.to_bytes()),
        }
    }
}

fn mock_worker_info() -> WorkerInfo {
    let data = include_str!("mock_node_info.json");
    serde_json::from_str(data).unwrap()
}
