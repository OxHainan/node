use anyhow::{anyhow, Result};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{error, info};

use crate::api_key_store::ApiKeyStore;

/// Admin interface for managing API keys
pub struct AdminInterface {
    /// API key store
    api_key_store: Arc<ApiKeyStore>,
}

impl AdminInterface {
    /// Create a new admin interface
    pub fn new(api_key_store: Arc<ApiKeyStore>) -> Self {
        Self { api_key_store }
    }

    /// Start the admin HTTP server
    pub async fn start(&self, bind_address: &str) -> Result<()> {
        let addr: SocketAddr = bind_address.parse()?;
        let api_key_store = self.api_key_store.clone();

        let make_service = make_service_fn(move |_| {
            let api_key_store = api_key_store.clone();

            async move {
                Ok::<_, hyper::Error>(service_fn(move |req| {
                    let api_key_store = api_key_store.clone();

                    async move { handle_admin_request(req, api_key_store).await }
                }))
            }
        });

        let server = Server::bind(&addr).serve(make_service);
        info!("mp Admin Interface listening on {}", addr);

        server
            .await
            .map_err(|e| anyhow!("Admin server error: {}", e))
    }
}

/// Handle admin API requests
async fn handle_admin_request(
    req: Request<Body>,
    api_key_store: Arc<ApiKeyStore>,
) -> Result<Response<Body>, hyper::Error> {
    match (req.method(), req.uri().path()) {
        // Add new API key
        (&Method::POST, "/admin/api-keys") => {
            let body_bytes = hyper::body::to_bytes(req.into_body()).await?;
            let body_str = String::from_utf8_lossy(&body_bytes);

            match serde_json::from_str::<serde_json::Value>(&body_str) {
                Ok(json) => {
                    let api_key = json.get("api_key").and_then(|k| k.as_str()).unwrap_or("");

                    let address = json.get("address").and_then(|a| a.as_str()).unwrap_or("");

                    if api_key.is_empty() || address.is_empty() {
                        return Ok(bad_request_response("Missing api_key or address"));
                    }

                    match api_key_store.add_api_key(api_key, address).await {
                        Ok(_) => {
                            let response = json!({
                                "success": true,
                                "message": "API key added successfully",
                                "api_key": api_key,
                                "address": address
                            });

                            Ok(json_response(StatusCode::CREATED, &response))
                        }
                        Err(e) => {
                            error!("Failed to add API key: {}", e);
                            Ok(internal_error_response(&format!(
                                "Failed to add API key: {}",
                                e
                            )))
                        }
                    }
                }
                Err(e) => Ok(bad_request_response(&format!("Invalid JSON: {}", e))),
            }
        }

        // Delete API key
        (&Method::DELETE, path) if path.starts_with("/admin/api-keys/") => {
            let api_key = path.trim_start_matches("/admin/api-keys/");

            match api_key_store.remove_api_key(api_key).await {
                Ok(_) => {
                    let response = json!({
                        "success": true,
                        "message": "API key removed successfully",
                        "api_key": api_key
                    });

                    Ok(json_response(StatusCode::OK, &response))
                }
                Err(e) => {
                    error!("Failed to remove API key: {}", e);
                    Ok(internal_error_response(&format!(
                        "Failed to remove API key: {}",
                        e
                    )))
                }
            }
        }

        // List all API keys
        (&Method::GET, "/admin/api-keys") => match api_key_store.list_api_keys().await {
            Ok(keys) => {
                let key_list = keys
                    .into_iter()
                    .map(|(key, address, nonce)| {
                        json!({
                            "api_key": key,
                            "address": address,
                            "nonce": nonce
                        })
                    })
                    .collect::<Vec<_>>();

                let response = json!({
                    "success": true,
                    "api_keys": key_list
                });

                Ok(json_response(StatusCode::OK, &response))
            }
            Err(e) => {
                error!("Failed to list API keys: {}", e);
                Ok(internal_error_response(&format!(
                    "Failed to list API keys: {}",
                    e
                )))
            }
        },

        // Not found for all other routes
        _ => Ok(not_found_response()),
    }
}

/// Create a JSON response
fn json_response(status: StatusCode, data: &serde_json::Value) -> Response<Body> {
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Body::from(data.to_string()))
        .unwrap_or_default()
}

/// Create a "bad request" response
fn bad_request_response(message: &str) -> Response<Body> {
    let response = json!({
        "success": false,
        "error": message
    });

    json_response(StatusCode::BAD_REQUEST, &response)
}

/// Create an "internal error" response
fn internal_error_response(message: &str) -> Response<Body> {
    let response = json!({
        "success": false,
        "error": message
    });

    json_response(StatusCode::INTERNAL_SERVER_ERROR, &response)
}

/// Create a "not found" response
fn not_found_response() -> Response<Body> {
    let response = json!({
        "success": false,
        "error": "Not found"
    });

    json_response(StatusCode::NOT_FOUND, &response)
}
