// pub mod api;
pub mod config;
pub mod pool;

use anyhow::Result;
use mp_common::types::TransactionStatusWithProof;
use mp_common::types::{Transaction, TransactionResponse};
use mp_consensus::ConsensusEngine;
use mp_poc::bls::SignedAggregate;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Transaction pool interface
#[async_trait::async_trait]
pub trait TransactionPool: Send + Sync {
    /// Start the transaction pool
    async fn start(&self) -> Result<()>;

    /// Stop the transaction pool
    fn stop(&self) -> Result<()>;

    /// Submit a transaction to the pool
    async fn submit_transaction(&self, transaction: Transaction) -> Result<TransactionResponse>;

    /// Get a channel for pending transactions
    async fn get_pending_tx_channel(&self) -> mpsc::Receiver<Transaction>;

    /// Get the status of a transaction
    async fn get_transaction_status(&self, tx_id: &Uuid) -> Result<TransactionStatusWithProof>;

    /// Update the result of a transaction
    async fn update_transaction_result(
        &self,
        tx_id: &Uuid,
        result: serde_json::Value,
        signed_aggregate: SignedAggregate,
    ) -> Result<()>;

    async fn get_transaction_proof(&self, tx_id: &Uuid) -> Option<serde_json::Value>;

    /// Get the result of a transaction
    async fn get_transaction_result(
        &self,
        tx_id: &Uuid,
    ) -> Result<(serde_json::Value, Option<serde_json::Value>)>;
}

/// Create a new transaction pool based on the configuration
pub fn create_transaction_pool(
    config: config::MempoolConfig,
    consensus_engine: Box<dyn ConsensusEngine>,
) -> Result<Arc<dyn TransactionPool>> {
    let pool = pool::BasicTransactionPool::new(config, consensus_engine)?;
    Ok(Arc::new(pool))
}
