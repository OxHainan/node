use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Transaction error: {0}")]
    Transaction(String),

    #[error("Consensus error: {0}")]
    Consensus(String),

    #[error("Compute error: {0}")]
    Compute(String),

    #[error("State error: {0}")]
    State(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}
