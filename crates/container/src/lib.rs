//! Container module for mp blockchain

pub mod config;
pub mod cvm;
pub mod docker;

use anyhow::Result;
use config::default_tappd_host;
use dstack::types::{AccessControl, AgentConfiguration, AuthorizationType, CreateAction, PricingModel, RequestId};
use mp_common::{
    types::{Transaction, TransactionType},
    utils::h128_to_uuid,
    TransactionResponse, H128,
};
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Method, StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fmt::Debug;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};
use uuid::Uuid;

pub use dstack::{compose::DockerCompose, types::CreateVmRequest, TappdClientT};

/// Container information structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInfo {
    /// Container ID
    pub contract_id: H128,
    /// Container name
    pub name: String,
    /// Network address for connecting to the container
    pub address: SocketAddr,
    /// Current container status
    pub status: ContainerStatus,
    pub instance_id: String,
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerDetail {
    pub agent_name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub pricing: PricingModel,
    pub daily_call_quote: u16,
    pub access: AccessControl,
    pub authorization_type: AuthorizationType,
    #[serde(flatten)]
    pub action: CreateAction,
    #[serde(flatten)]
    pub info: ContainerInfo,
}

/// Container status enumeration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ContainerStatus {
    /// Container is starting
    Starting,
    /// Container is running
    Running,
    /// Container is stopping
    Stopping,
    /// Container is stopped
    Stopped,
    /// Container encountered an error
    Error(String),
}

/// Container environment trait defining common operations for container management
#[async_trait::async_trait]
pub trait ContainerEnvironment: Send + Sync + Debug + 'static {
    async fn create_container(&self, req: AgentConfiguration) -> Result<ContainerInfo>;

    /// Execute a transaction in the container environment
    async fn execute_transaction(&self, transaction: Transaction) -> Result<Transaction> {
        info!("[DOCKER] Executing transaction: {:?}", transaction.id);
        match &transaction.tx_type {
            TransactionType::Request(id, path) => {
                let container_info = match self.get_container(&h128_to_uuid(&id)).await {
                    Ok(info) => info,
                    Err(e) => return handle_internal_error(&transaction, e),
                };
                if container_info.info.status != ContainerStatus::Running {
                    return handle_internal_error(&transaction, "Container is not running");
                }
                match execute_api_request(
                    format!("http://{}/{}", container_info.info.address, path),
                    transaction.payload.clone(),
                    transaction.method.clone(),
                    transaction.header.clone(),
                )
                .await
                {
                    Ok(response) => handle_response(&transaction, response),
                    Err(e) => {
                        error!("[DOCKER] Failed to execute API request: {}", e);
                        handle_internal_error(&transaction, e)
                    }
                }
            }
            TransactionType::CreateContainer => {
                let req = match serde_json::from_slice::<CreateVmRequest>(&transaction.payload) {
                    Ok(req) => req,
                    Err(e) => return handle_internal_error(&transaction, e),
                };
            
                match req.action {
                    CreateAction::Agent(agent) => {
                        info!("[DOCKER] Creating new container: {:?}", agent.name);
                        match self.create_container(agent).await {
                            Ok(res) => handle_internal_response(&transaction, res),
                            Err(e) => handle_internal_error(&transaction, e),
                        }
                    },
                    CreateAction::External(host) => todo!(),
                }
            }
            TransactionType::StopContainer => {
                let req = match serde_json::from_slice::<RequestId>(&transaction.payload) {
                    Ok(req) => req,
                    Err(e) => return handle_internal_error(&transaction, e),
                };

                match self.stop_container(&req.id()).await {
                    Ok(_) => handle_internal_response(
                        &transaction,
                        format!("Container {} stopped successfully", req.id()),
                    ),
                    Err(e) => handle_internal_error(&transaction, e),
                }
            }
            TransactionType::ListContainers => match self.get_running_containers().await {
                Ok(containers) => {
                    println!("[DOCKER] List containers: {:?}", containers);
                    handle_internal_response(&transaction, containers)
                },
                Err(e) => handle_internal_error(&transaction, e),
            },
            TransactionType::StartContainer => {
                let req = match serde_json::from_slice::<RequestId>(&transaction.payload) {
                    Ok(req) => req,
                    Err(e) => return handle_internal_error(&transaction, e),
                };

                match self.start_container(&req.id()).await {
                    Ok(res) => handle_internal_response(&transaction, res),
                    Err(e) => handle_internal_error(&transaction, e),
                }
            }
            TransactionType::RemoveContainer => {
                let req = match serde_json::from_slice::<RequestId>(&transaction.payload) {
                    Ok(req) => req,
                    Err(e) => return handle_internal_error(&transaction, e),
                };

                match self.remove_container(&req.id()).await {
                    Ok(_) => handle_internal_response(
                        &transaction,
                        format!("Container {} removed successfully", req.id()),
                    ),
                    Err(e) => handle_internal_error(&transaction, e),
                }
            }
            _ => {
                // For other transaction types, just return as is for now
                warn!(
                    "[DOCKER] Unsupported transaction type: {:?}",
                    transaction.id
                );
                Ok(transaction)
            }
        }
    }

    /// Start a container for a specific module
    async fn start_container(&self, vm_id: &Uuid) -> Result<ContainerInfo>;

    /// Stop a specific container
    async fn stop_container(&self, vm_id: &Uuid) -> Result<()>;

    /// Remove a specific container
    async fn remove_container(&self, vm_id: &Uuid) -> Result<()>;

    async fn get_container(&self, vm_id: &Uuid) -> Result<ContainerDetail>;

    /// Get the status of a specific container
    async fn get_container_status(&self, vm_id: &Uuid) -> Result<ContainerStatus>;

    /// Get all running containers
    async fn get_running_containers(&self) -> Result<Vec<ContainerDetail>>;
}

/// Create a new container environment based on the configuration
pub async fn create_container_environment(
    config: config::ContainerConfig,
) -> Result<(Arc<Mutex<dyn TappdClientT>>, Arc<dyn ContainerEnvironment>)> {
    match config.container_mode {
        config::ContainerMode::Simulated => {
            info!("Creating simulated container environment");
            let env = Arc::new(docker::DockerContainerEnvironment::new(
                config.tappd_host.unwrap_or(default_tappd_host()),
            ));
            // env.init_vms().await?;
            Ok((env.get_tappd_client(), env))
        }
        config::ContainerMode::CVM => {
            info!("Creating Docker container environment");
            let env = Arc::new(
                cvm::ContainerVirtureManager::new(
                    config.teepod_host,
                    config
                        .tappd_host
                        .unwrap_or("/var/run/tappd.sock".to_string()),
                )
                .await?,
            );
            Ok((env.get_tappd_client(), env))
        }
    }
}

/// Execute an API request in a container
#[derive(Debug)]
pub struct ApiResponse {
    pub status: StatusCode,
    pub body: serde_json::Value,
    pub headers: HeaderMap<HeaderValue>,
}

async fn execute_api_request(
    target_url: String,
    payload: Vec<u8>,
    method: Method,
    headers: HeaderMap<HeaderValue>,
) -> Result<ApiResponse> {
    // Log detailed request information
    info!("[DOCKER] ====== API Request Details ======");
    info!("[DOCKER] Method: {}", method);
    // info!("[DOCKER] Path: {}", payload.path);
    info!("[DOCKER] Headers: {:?}", headers);
    info!(
        "[DOCKER] Request Body: {}",
        String::from_utf8_lossy(&payload)
    );

    // Create reqwest client
    let client = reqwest::Client::new();

    // let target_url = format!("http://{}/{}", base_url, payload.path);
    info!("[DOCKER] Forwarding request to: {}", target_url);
    // Forward request to web2_style contract at port 8080
    let response = client
        .request(method, target_url)
        .headers(headers)
        .header("Content-Type", "application/json")
        .body(payload)
        .send()
        .await?;
    let status = response.status();
    let headers = response.headers().clone();
    // Log response details0
    info!("[DOCKER] ====== API Response Details ======");
    info!("[DOCKER] Status: {}", status);
    info!("[DOCKER] Response Headers: {:?}", headers);

    // Get response bytes and log them
    let response_bytes = response.bytes().await?.to_vec();
    info!(
        "[DOCKER] Response Body: {}",
        String::from_utf8_lossy(&response_bytes)
    );

    info!("[DOCKER] API request completed successfully");

    Ok(ApiResponse {
        status,
        body: serde_json::from_slice(&response_bytes).unwrap(),
        headers,
    })
}

fn handle_internal_error(transaction: &Transaction, error: impl ToString) -> Result<Transaction> {
    let response_data = json!({
        "status_code": 500,
        "error": error.to_string(),
    });

    let result_tx = mp_common::utils::create_transaction(
        transaction.tx_type.clone(),
        serde_json::to_vec(&response_data).unwrap(),
        None,
        transaction.method.clone(),
        Default::default(),
    );

    info!("[DOCKER] Created result transaction: {:?}", result_tx.id);
    Ok(result_tx)
}

fn handle_internal_response<S: Serialize>(
    transaction: &Transaction,
    response: S,
) -> Result<Transaction> {
    let response_data = json!({
        "status_code": 200,
        "result": response,
    });

    let result_tx = mp_common::utils::create_transaction(
        transaction.tx_type.clone(),
        serde_json::to_vec(&response_data).unwrap(),
        None,
        transaction.method.clone(),
        Default::default(),
    );

    info!("[DOCKER] Created result transaction: {:?}", result_tx.id);
    Ok(result_tx)
}

fn handle_response(transaction: &Transaction, response: ApiResponse) -> Result<Transaction> {
    let mut response_data: TransactionResponse = serde_json::from_value(response.body).unwrap();
    if response_data.status_code.is_none() {
        response_data.status_code = Some(response.status.as_u16() as u32);
    }

    let result_tx = mp_common::utils::create_transaction(
        transaction.tx_type.clone(),
        serde_json::to_vec(&response_data).unwrap(),
        None,
        transaction.method.clone(),
        response.headers.clone(),
    );

    info!("[DOCKER] Created result transaction: {:?}", result_tx.id);
    Ok(result_tx)
}

pub mod utils {
    use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
    use sha1::Digest;
    use uuid::Uuid;
    pub fn string_to_uuid(target: Option<String>) -> Uuid {
        if let Some(target) = target {
            let escaped_str = utf8_percent_encode(&target, NON_ALPHANUMERIC).to_string();
            let buffer = escaped_str.into_bytes();

            let hash = sha1::Sha1::digest(&buffer);
            let mut uuid_bytes = [0u8; 16];
            uuid_bytes.copy_from_slice(&hash[..16]);
            uuid_bytes[6] = (uuid_bytes[6] & 0x0F) | 0x40; // 版本号 4
            uuid_bytes[8] = (uuid_bytes[8] & 0x3F) | 0x80; // 变体
            Uuid::from_bytes(uuid_bytes)
        } else {
            Uuid::new_v4()
        }
    }
}
