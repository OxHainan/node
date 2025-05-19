//! Transaction-related functionality for the mp SDK

use mp_common::types::{Transaction, TransactionType};
use mp_common::utils::create_transaction;
use reqwest::header::HeaderMap;
use reqwest::Method;
use serde::{Deserialize, Serialize};

/// Transaction status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TransactionStatus {
    /// Transaction is pending (in mempool)
    Pending,
    /// Transaction is being processed
    Processing,
    /// Transaction has been confirmed with optional execution result
    Confirmed(Option<serde_json::Value>),
    /// Transaction has failed
    Failed(String),
    /// Transaction not found
    NotFound,
}

/// Builder for creating transactions
#[derive(Debug, Clone)]
pub struct TransactionBuilder {
    tx_type: TransactionType,
    payload: Vec<u8>,
    sender: Option<String>,
}

impl TransactionBuilder {
    /// Create a new transaction builder
    pub fn new(tx_type: TransactionType) -> Self {
        Self {
            tx_type,
            payload: Vec::new(),
            sender: None,
        }
    }

    /// Create a new API request transaction builder
    pub fn api_request(handler: String) -> Option<Self> {
        match TransactionType::parse(&handler) {
            Some(tx_type) => Some(Self::new(tx_type)),
            None => None,
        }
    }

    /// Create a new state change transaction builder
    pub fn state_change() -> Self {
        Self::new(TransactionType::StateChange)
    }

    /// Create a new scheduled task transaction builder
    pub fn scheduled_task() -> Self {
        Self::new(TransactionType::ScheduledTask)
    }

    /// Set the transaction payload
    pub fn payload<T: Serialize>(mut self, payload: &T) -> Result<Self, serde_json::Error> {
        self.payload = serde_json::to_vec(payload)?;
        Ok(self)
    }

    /// Set the transaction payload from raw bytes
    pub fn payload_raw(mut self, payload: Vec<u8>) -> Self {
        self.payload = payload;
        self
    }

    /// Set the transaction sender
    pub fn sender(mut self, sender: impl Into<String>) -> Self {
        self.sender = Some(sender.into());
        self
    }

    /// Build the transaction
    pub fn build(self) -> Transaction {
        create_transaction(
            self.tx_type,
            self.payload,
            self.sender,
            Method::POST,
            HeaderMap::new(),
        )
    }
}

/// Helper functions for transactions
pub mod helpers {
    use super::*;
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

    /// Encode transaction payload as base64
    pub fn encode_payload(payload: &[u8]) -> String {
        BASE64.encode(payload)
    }

    /// Decode transaction payload from base64
    pub fn decode_payload(payload: &str) -> Result<Vec<u8>, base64::DecodeError> {
        BASE64.decode(payload)
    }

    /// Parse transaction payload as JSON
    pub fn parse_payload<T: for<'de> Deserialize<'de>>(
        payload: &[u8],
    ) -> Result<T, serde_json::Error> {
        serde_json::from_slice(payload)
    }
}
