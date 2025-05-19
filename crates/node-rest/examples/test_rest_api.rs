use anyhow::Result;
use mp_executor::core::{DummyExecutionEngine, ExecutionRequest};
use mp_mempool::TransactionPool;
use mp_node_rest::{ApiKeyStore, IntegratedRestApi, RestApiConfig};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Create a test configuration
    let config = RestApiConfig {
        key_store_path: "./test_api_keys.db".to_string(),
        rest_bind_address: "127.0.0.1:3000".to_string(),
        admin_bind_address: "127.0.0.1:3001".to_string(),
        tx_timeout: 30,
    };

    // Initialize API key store
    let api_key_store = ApiKeyStore::new(&config.key_store_path).await?;
    let api_key_store = Arc::new(api_key_store);

    // Generate a test API key
    let test_address = "0x1234567890abcdef1234567890abcdef12345678";
    let api_key = api_key_store
        .generate_key(Some("Test Key".to_string()), test_address)
        .await?;
    println!("Generated test API key: {}", api_key);

    // Create dummy transaction pool (for testing)
    let mempool_config = mp_mempool::config::MempoolConfig {
        max_transactions: 1000,
        api_address: "127.0.0.1:8545".to_string(),
        max_tx_size: 1048576,
        tx_timeout: 60,
    };

    // Note: You would need to create a real consensus engine and transaction pool in a production environment
    // This is simplified for demonstration purposes
    let tx_pool = TransactionPool::new_test_pool(mempool_config);

    // Create execution channel
    let (exec_sender, mut exec_receiver) = mpsc::channel::<ExecutionRequest>(100);

    // Create integrated REST API
    let rest_api = IntegratedRestApi::new(
        config,
        exec_sender.clone(),
        Arc::new(tx_pool) as Arc<dyn TransactionPool + Send + Sync>,
        api_key_store.clone(),
    );

    // Process execution requests in a separate task (simulating the executor)
    tokio::spawn(async move {
        println!("Starting mock execution engine");

        // In a real application, this would be the actual execution engine
        // For testing, we'll just log the requests and send back dummy responses
        while let Some(request) = exec_receiver.recv().await {
            println!("Received execution request: {:?}", request);

            // Create a dummy response
            let response = serde_json::json!({
                "status": 200,
                "body": {
                    "success": true,
                    "message": "Request processed successfully",
                    "request_id": request.tx_hash,
                    "module": request.module_id,
                    "handler": request.handler,
                    "input": request.input,
                },
                "headers": {
                    "Content-Type": "application/json"
                }
            });

            // Here, we would normally update the transaction result in the pool
            // For testing, you can print the response
            println!(
                "Mock response: {}",
                serde_json::to_string_pretty(&response).unwrap()
            );
        }
    });

    // Start the REST API in the main task
    println!("Starting integrated REST API on 127.0.0.1:3000");
    println!("Admin interface on 127.0.0.1:3001");
    println!("Press Ctrl+C to stop");

    // In a real application, you would start multiple components
    // For this test, we'll just start the REST API
    rest_api.start().await?;

    Ok(())
}
