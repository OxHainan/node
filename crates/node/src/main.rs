use anyhow::Result;
use clap::Parser;
use config::{Config, File};
use dstack::TdxQuoteArgs;
use mp_common::types::TransactionStatusWithProof;
// Transaction type is used in the code but only through imported functions
// 移除 mp_compute 导入，使用 mp_container 代替
use mp_consensus::{config::ConsensusConfig, create_consensus_engine};
use mp_container::{config::ContainerConfig, create_container_environment};
use mp_executor::{
    config::ExecutorConfig, core::ExecutionResponse, create_execution_engine, ExecutionEngineType,
};
use mp_mempool::{config::MempoolConfig, create_transaction_pool};
use mp_network::{config::NetworkConfig, create_network};
use mp_node_rest::PoCQuote;
use mp_state::{config::StateConfig, create_state_storage};
use mp_poc::PoC;
use serde::Deserialize;
use std::sync::Arc;
use std::{collections::HashMap, path::Path};
use tokio::sync::Mutex;

use tracing::{debug, error, info, Level};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

/// mp Node - A blockchain platform for Web2-style smart contracts using Docker
#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// Path to the configuration file
    #[clap(short, long, default_value = "config.toml")]
    config: String,

    /// Log level
    #[clap(short, long, default_value = "info")]
    log_level: String,

    /// Enable REST API
    #[clap(long)]
    with_rest_api: bool,
}

/// Node configuration
#[derive(Debug, Deserialize, Clone)]
struct NodeConfig {
    node: NodeSettings,
    consensus: ConsensusConfig,
    mempool: MempoolConfig,
    // 移除旧的 compute 配置，使用 container 配置代替
    container: ContainerConfig, // 新的容器配置
    executor: ExecutorConfig,
    state: StateConfig,
    network: NetworkConfig,
    rest_api: Option<mp_node_rest::RestApiConfig>,
    security: SecurityConfig,
}

/// Node settings
#[derive(Debug, Deserialize, Clone)]
struct NodeSettings {
    node_id: u64,
    log_level: String,
}

/// Security configuration
#[derive(Debug, Deserialize, Clone)]
struct SecurityConfig {
    enable_poc: bool,
    enable_pom: bool,
}

fn main() -> Result<()> {
    dotenv::dotenv().ok();
    // Parse command line arguments
    let args = Args::parse();

    // Load configuration
    let config_path = Path::new(&args.config);
    let config = load_config(config_path)?;

    // Initialize logging
    let log_level = match config.node.log_level.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };

    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::new("info,hyper_util=off"))
        .with_max_level(log_level)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting mp Node");
    info!("Node ID: {}", config.node.node_id);
    info!("Using configuration file: {}", args.config);

    // Create data directories
    std::fs::create_dir_all("./data/raft")?;
    std::fs::create_dir_all("./data/state_root")?;

    // Start the runtime
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async {
            // Initialize and start node components
            if let Err(e) = run_node(config.clone(), args.with_rest_api).await {
                error!("Node failed: {}", e);
                return Err(e);
            }

            // Wait for shutdown signal
            tokio::signal::ctrl_c().await?;
            info!("Shutting down mp Node");
            Ok::<(), anyhow::Error>(())
        })?;

    Ok(())
}

/// Load configuration from file
fn load_config(config_path: &Path) -> Result<NodeConfig> {
    let config = Config::builder()
        .add_source(File::from(config_path))
        .build()?;

    let node_config: NodeConfig = config.try_deserialize()?;
    Ok(node_config)
}

/// Run the node with the given configuration
async fn run_node(config: NodeConfig, with_rest_api: bool) -> Result<()> {
    // Initialize consensus engine
    info!("Initializing consensus engine");
    // Clone the consensus config so we can use it again later
    let consensus_config = config.consensus.clone();
    let mut consensus_engine = create_consensus_engine(consensus_config)?;

    // Start consensus engine
    consensus_engine.start().await?;

    // Get confirmed transaction channel from consensus
    let confirmed_tx_rx = consensus_engine.get_confirmed_tx_channel().await;

    // Initialize transaction pool with consensus engine
    info!("Initializing transaction pool");
    let tx_pool = create_transaction_pool(config.mempool, consensus_engine)?;
    tx_pool.start().await?;

    // Create a new consensus engine for other components
    let _consensus_engine = create_consensus_engine(config.consensus)?;

    // Initialize container environment
    info!("Initializing container environment");
    let (tappd_client, container_env) = create_container_environment(config.container).await?;

    // Initialize state storage
    info!("Initializing state storage");
    let state_storage = create_state_storage(config.state)?;
    state_storage.start()?;

    // Initialize P2P network
    info!("Initializing P2P network");
    let network = create_network(config.network)?;
    network.start()?;

    info!("mp Node is running with Raft consensus");

    // Clone the executor config before we move it
    let mut executor_config = config.executor.clone();

    // Set execution engine type and container environment in executor config
    executor_config.engine_type = ExecutionEngineType::Network;
    executor_config.container_environment = Some(container_env.clone());

    // Initialize execution engine
    info!("Initializing execution engine with compute environment");
    let exec_engine = create_execution_engine(executor_config.clone()).await?;

    // Setup execution bridge for cross-process communication
    let mut bridge = mp_executor::bridge::ExecutionBridge::new(
        exec_engine,
        executor_config.worker_threads,
        1000,
    );

    // Initialize PoC
    let mock_poc = Arc::new(mp_poc::mock::MockPoC::new());
    let aggregate_public_key = mock_poc.aggregate_public_key()?;
    info!(
        "Aggregate public key: {:?}",
        hex::encode(aggregate_public_key.to_bytes())
    );

    let poc_quote = tappd_client
        .lock()
        .await
        .tdx_quote(TdxQuoteArgs {
            report_data: aggregate_public_key.to_bytes().to_vec(),
            hash_algorithm: "keccak256".to_string(),
            ..Default::default()
        })
        .await?;

    // Start the bridge and get result receiver
    let mut exec_result_rx = bridge.start(config.executor.worker_threads).await?;
    let exec_request_tx = bridge.get_request_sender();

    // Create a channel for forwarding execution results to the REST API
    let (exec_result_forward_tx, mut exec_result_forward_rx) =
        tokio::sync::mpsc::channel::<ExecutionResponse>(1000);

    let api_result_tx = Arc::new(Mutex::new(HashMap::new()));
    // Start the integrated REST API if requested
    if with_rest_api {
        info!("Initializing integrated RESTful API");

        // Only proceed if REST API config is available
        if let Some(rest_config) = config.rest_api.clone() {
            // Initialize API key store
            let api_key_store =
                mp_node_rest::ApiKeyStore::new(&rest_config.key_store_path).await?;

            // Create admin interface with the API key store
            let admin_interface = mp_node_rest::AdminInterface::new(
                api_key_store.clone(),
                tappd_client.clone(),
                PoCQuote::new(poc_quote, aggregate_public_key),
            );
            let admin_bind_address = rest_config.admin_bind_address.clone();

            // Create a channel for direct execution requests
            let (exec_sender, mut exec_receiver) = tokio::sync::mpsc::channel(1000);

            // Clone exec_request_tx for use in the task
            let exec_request_tx_for_task = exec_request_tx.clone();

            // Clone tx_pool for use in REST API and result processor
            let tx_pool_clone =
                tx_pool.clone() as Arc<dyn mp_mempool::TransactionPool + Send + Sync>;

            // Create the integrated REST API
            let rest_api = mp_node_rest::IntegratedRestApi::new(
                rest_config,
                exec_sender,
                tx_pool_clone,
                api_key_store,
            );

            // Start admin interface in a separate task
            tokio::spawn(async move {
                if let Err(e) = admin_interface.start(&admin_bind_address).await {
                    error!("Admin interface error: {}", e);
                }
            });

            // Start REST API in a separate task
            let rest_api_clone = rest_api;
            tokio::spawn(async move {
                if let Err(e) = rest_api_clone.start().await {
                    error!("REST API error: {}", e);
                }
            });
            let api_result_tx_clone = api_result_tx.clone();
            // Process execution requests from the REST API
            tokio::spawn(async move {
                info!("Starting REST API execution request processing");

                while let Some((request, sender)) = exec_receiver.recv().await {
                    let tx_hash = request.tx_hash.clone();
                    info!("Processing REST API execution request for tx: {}", tx_hash);

                    // Do not create a new oneshot channel
                    // The REST API has already created a channel and stored the receiver in the result_receivers map
                    // We only need to ensure the result_sender field in the execution request is correctly passed
                    api_result_tx_clone.lock().await.insert(tx_hash, sender);
                    // Forward request to executor
                    if let Err(e) = exec_request_tx_for_task.send(request).await {
                        error!("Failed to forward execution request: {}", e);
                        continue;
                    }
                }
            });

            // Process execution results for REST API requests
            let tx_pool_for_results = tx_pool.clone();
            tokio::spawn(async move {
                info!("Starting REST API execution result processing");

                while let Some(response) = exec_result_forward_rx.recv().await {
                    let tx_hash = &response.result.metadata.tx_hash;
                    info!("Received execution result for REST API tx: {}", tx_hash);

                    // We no longer need to handle REST API result sending here
                    // Because the execution engine will directly send the result through the ExecutionRequest's result_sender
                    // Here we only log, confirming we received the execution result
                    debug!("Received execution result for REST API tx: {}, result will be sent directly by executor", tx_hash);

                    // 2. Update the transaction status in the transaction pool to confirmed
                    // Regardless of whether there are waiting handlers, we need to update the transaction status
                    if let Err(e) = tx_pool_for_results
                        .update_transaction_result(
                            tx_hash,
                            response.result.output.output,
                            response.signed_aggregate.clone(),
                        )
                        .await
                    {
                        error!("Failed to update transaction result in mempool: {}", e);
                    } else {
                        debug!("Updated transaction result in mempool for tx: {}", tx_hash);
                    }

                    // Log, indicating the result is stored in the mempool
                    debug!("Result stored in mempool for tx: {}", tx_hash);
                }
            });
        } else {
            error!("REST API configuration missing in config file");
        }
    }

    // Process submitted transactions, execute then consensus
    let tx_pool_clone = tx_pool.clone();
    let exec_request_tx_clone = exec_request_tx.clone();
    let _tx_processing_handle = tokio::spawn(async move {
        info!("Starting transaction processing (execute-then-consensus model)");

        // Get pending transactions
        let mut pending_tx_rx = tx_pool_clone.get_pending_tx_channel().await;

        while let Some(tx) = pending_tx_rx.recv().await {
            let tx_id = tx.id;
            info!("Processing new transaction: {}", tx_id);

            // Create execution request
            let request = mp_executor::core::ExecutionRequest {
                header: tx.header,
                transaction_type: tx.tx_type,
                input: tx.payload,
                tx_hash: tx_id,
                method: tx.method,
            };

            // 1. Send to execution module first
            info!("Sending transaction {} to executor", tx_id);
            if let Err(e) = exec_request_tx_clone.send(request).await {
                error!("Failed to send transaction to executor: {}", e);
                continue;
            }

            // Note: Execution result will be handled by another task and then submitted to consensus
            // This allows transactions to be submitted faster, without waiting for execution to complete
        }
    });

    // Process execution results, then submit to consensus
    let tx_pool_clone = tx_pool.clone();

    // Create a task to process execution results and forward
    let (main_forward_tx, mut main_forward_rx) =
        tokio::sync::mpsc::channel::<ExecutionResponse>(1000);

    // Start a task to process main_forward_rx received execution results
    let tx_pool_for_main = tx_pool.clone();
    tokio::spawn(async move {
        info!("Starting main execution result processing");

        while let Some(response) = main_forward_rx.recv().await {
            let tx_hash = &response.result.metadata.tx_hash;
            info!("Processing main execution result for tx: {}", tx_hash);

            // Update transaction status in the transaction pool
            if let Err(e) = tx_pool_for_main
                .update_transaction_result(
                    tx_hash,
                    response.result.output.output,
                    response.signed_aggregate.clone(),
                )
                .await
            {
                error!(
                    "Failed to update transaction result in main processor: {}",
                    e
                );
            } else {
                info!(
                    "Successfully updated transaction result in main processor for {}",
                    tx_hash
                );
            }
        }
    });

    // Use the previously created channel - do not recreate
    let api_result_tx_clone = api_result_tx.clone();
    // Main execution result processing task
    let _result_processing_handle = tokio::spawn(async move {
        info!("Starting execution result processing with consensus");
        while let Some(result) = exec_result_rx.recv().await {
            let tx_hash = &result.metadata.tx_hash;
            info!("Received execution result for tx: {}", tx_hash);
            let result_output = result.output.output.clone();
            info!("poc calc result_output: {:?}", result_output);
            info!("poc calc input: {:?}", hex::encode(result.input.clone()));
            info!(
                "poc calc input: {:?}",
                serde_json::from_slice::<serde_json::Value>(&result.input).unwrap()
            );
            // Forward result to REST API processing task
            let execution_response = ExecutionResponse {
                result: result.clone(),
                signed_aggregate: mock_poc
                    .generate_aggregate(vec![(
                        result.input.clone(),
                        serde_json::to_vec(&result_output).unwrap(),
                    )])
                    .unwrap(),
            };
            let headers = result.headers.clone();

            // If REST API is enabled, prioritize forwarding execution result to REST API processing channel
            // This is the key to active notification - ensure sending notification before updating the transaction pool
            if with_rest_api {
                // First, forward execution result to REST API
                if let Err(e) = exec_result_forward_tx
                    .send(execution_response.clone())
                    .await
                {
                    error!(
                        "Failed to forward execution result to REST API channel: {}",
                        e
                    );
                } else {
                    info!(
                        "Successfully forwarded execution result to REST API channel for tx: {}",
                        result.metadata.tx_hash
                    );
                }

                if let Some(sender) = api_result_tx_clone.lock().await.remove(tx_hash) {
                    println!("Found sender for tx: {}", tx_hash);
                    let poc: PoC = execution_response
                        .signed_aggregate
                        .clone()
                        .try_into()
                        .unwrap();

                    let status = execution_response.result.output.status_code.unwrap_or(200) as u16;
                    if 200 <= status && status < 300 {
                        if let Err(e) = sender.send(TransactionStatusWithProof::Confirmed(
                            result_output.clone(),
                            status,
                            Some(headers),
                            Some(serde_json::json!(poc)),
                        )) {
                            error!("Failed to send execution result to REST API: {:?}", e);
                        }
                    } else {
                        if let Err(e) = sender.send(TransactionStatusWithProof::Failed(
                            result_output.clone(),
                            status,
                            Some(headers),
                            Some(serde_json::json!(poc)),
                        )) {
                            error!("Failed to send execution result to REST API: {:?}", e);
                        }
                    }
                } else {
                    error!("Failed to get result to REST API, tx: {}", tx_hash);
                }
            }

            // Then forward to main processing channel
            if let Err(e) = main_forward_tx.send(execution_response.clone()).await {
                error!("Failed to forward execution result to main channel: {}", e);
            } else {
                debug!(
                    "Forwarded execution result to main channel for tx: {}",
                    result.metadata.tx_hash
                );
            }

            // IMPROVED EXECUTION RESULT TRACKING
            info!("EXECUTION RESULT TRACKING - Transaction: {}", tx_hash);
            info!(
                "Raw execution output: {}",
                serde_json::to_string_pretty(&result.output).unwrap_or_default()
            );

            // 2. Submit execution result to consensus module
            info!("Submitting execution result to consensus: {}", tx_hash);

            // Simplified implementation: directly update execution result in the transaction pool
            if let Err(e) = tx_pool_clone
                .update_transaction_result(
                    tx_hash,
                    result_output.clone(),
                    execution_response.signed_aggregate.clone(),
                )
                .await
            {
                error!("Failed to update transaction result for {}: {}", tx_hash, e);
                // Retry once
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                if let Err(e) = tx_pool_clone
                    .update_transaction_result(
                        tx_hash,
                        result_output,
                        execution_response.signed_aggregate.clone(),
                    )
                    .await
                {
                    error!(
                        "Second attempt failed to update transaction result for {}: {}",
                        tx_hash, e
                    );
                } else {
                    info!(
                        "Second attempt succeeded in updating transaction result for {}",
                        tx_hash
                    );
                }
            } else {
                info!("Successfully updated transaction result for {}", tx_hash);
            }
        }
    });

    Ok(())
}
