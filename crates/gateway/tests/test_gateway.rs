use anyhow::Result;
use reqwest::{Client, StatusCode};
use serde_json::json;
use std::time::Duration;
use tokio::time::sleep;

// This test requires a running mp node and gateway
// Run with: cargo test --test test_gateway -- --ignored
#[tokio::test]
#[ignore]
async fn test_gateway_integration() -> Result<()> {
    let client = Client::new();

    // 1. First, add an API key through the admin interface
    let api_key = format!("test-key-{}", chrono::Utc::now().timestamp());
    let address = "0x1234567890abcdef1234567890abcdef12345678"; // Example address

    let response = client
        .post("http://localhost:3001/admin/api-keys")
        .json(&json!({
            "api_key": api_key,
            "address": address
        }))
        .send()
        .await?;

    assert_eq!(response.status(), StatusCode::CREATED);
    println!("Added API key: {}", api_key);

    // 2. List API keys to verify
    let response = client
        .get("http://localhost:3001/admin/api-keys")
        .send()
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    let json_response = response.json::<serde_json::Value>().await?;
    println!("API keys: {}", json_response);

    // 3. Make a request through the gateway
    let response = client
        .get("http://localhost:3000/api/test")
        .header("X-API-Key", &api_key)
        .send()
        .await?;

    println!("Gateway response status: {}", response.status());
    println!("Gateway response body: {}", response.text().await?);

    // 4. Clean up - delete the API key
    let response = client
        .delete(&format!("http://localhost:3001/admin/api-keys/{}", api_key))
        .send()
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    println!("Deleted API key: {}", api_key);

    Ok(())
}

// This is a simple script to test the gateway manually
#[tokio::main]
async fn main() -> Result<()> {
    let client = Client::new();

    // 1. Add an API key
    let api_key = format!("test-key-{}", chrono::Utc::now().timestamp());
    let address = "0x1234567890abcdef1234567890abcdef12345678"; // Example address

    println!("Adding API key: {}", api_key);
    let response = client
        .post("http://localhost:3001/admin/api-keys")
        .json(&json!({
            "api_key": api_key,
            "address": address
        }))
        .send()
        .await?;

    println!(
        "Response: {} - {}",
        response.status(),
        response.text().await?
    );

    // 2. List API keys
    println!("\nListing API keys:");
    let response = client
        .get("http://localhost:3001/admin/api-keys")
        .send()
        .await?;

    println!(
        "Response: {} - {}",
        response.status(),
        response.text().await?
    );

    // 3. Make a request through the gateway
    println!("\nMaking request through gateway:");
    let response = client
        .get("http://localhost:3000/api/test")
        .header("X-API-Key", &api_key)
        .send()
        .await?;

    println!(
        "Response: {} - {}",
        response.status(),
        response.text().await?
    );

    // 4. Make a POST request through the gateway
    println!("\nMaking POST request through gateway:");
    let response = client
        .post("http://localhost:3000/api/users")
        .header("X-API-Key", &api_key)
        .json(&json!({
            "name": "Test User",
            "email": "test@example.com"
        }))
        .send()
        .await?;

    println!(
        "Response: {} - {}",
        response.status(),
        response.text().await?
    );

    // 5. Delete the API key
    println!("\nDeleting API key: {}", api_key);
    let response = client
        .delete(&format!("http://localhost:3001/admin/api-keys/{}", api_key))
        .send()
        .await?;

    println!(
        "Response: {} - {}",
        response.status(),
        response.text().await?
    );

    Ok(())
}
