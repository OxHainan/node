//! Simple example of using the mp SDK

use anyhow::Result;
use mp_sdk::{mpClient, TransactionBuilder};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create a client
    let mut client = mpClient::new("http://127.0.0.1:8545");

    // Get node info
    let node_info = client.get_node_info().await?;
    println!("Node info: {}", node_info);

    // Build and submit an API request transaction
    let api_payload = json!({
        "method": "get",
        "path": "/api/v1/users",
        "query": { "limit": 10 }
    });

    let transaction = TransactionBuilder::api_request()
        .payload(&api_payload)?
        .sender("example-client")
        .build();

    // Submit the transaction
    let tx_id = client.submit_transaction(transaction).await?;
    println!("Submitted transaction: {}", tx_id);

    // Wait for the transaction to be confirmed
    let status = client.wait_for_transaction(tx_id, 30).await?;
    println!("Transaction status: {:?}", status);

    // Send a direct API request
    let response = client
        .api_request("user-service", "/api/v1/users", &json!({ "limit": 10 }))
        .await?;
    println!("API response: {}", response);

    Ok(())
}
