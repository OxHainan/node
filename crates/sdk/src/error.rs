//! Error types for the mp SDK

use thiserror::Error;

/// Error type for the mp SDK
#[derive(Error, Debug)]
pub enum Error {
    /// Error communicating with the node
    #[error("Node communication error: {0}")]
    NodeCommunication(String),

    /// Error serializing or deserializing data
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Error with HTTP request
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Transaction submission error
    #[error("Transaction submission error: {0}")]
    TransactionSubmission(String),

    /// Transaction not found
    #[error("Transaction not found: {0}")]
    TransactionNotFound(String),

    /// Invalid transaction data
    #[error("Invalid transaction data: {0}")]
    InvalidTransactionData(String),

    /// Storage related errors
    #[error("Storage error: {0}")]
    Storage(String),

    /// IO errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Other errors
    #[error("Other error: {0}")]
    Other(String),
}

/// Result type for the mp SDK
pub type Result<T> = std::result::Result<T, Error>;
