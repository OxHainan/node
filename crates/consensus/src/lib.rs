pub mod config;
pub mod raft;

use anyhow::Result;
use mp_common::types::{Transaction, TransactionResponse};

use tokio::sync::mpsc;

/// Consensus engine interface
#[async_trait::async_trait]
pub trait ConsensusEngine: Send + Sync {
    /// Start the consensus engine
    async fn start(&mut self) -> Result<()>;

    /// Stop the consensus engine
    async fn stop(&mut self) -> Result<()>;

    /// Submit a transaction to the consensus engine
    async fn submit_transaction(&self, transaction: Transaction) -> Result<TransactionResponse>;

    /// Get a channel for confirmed transactions
    async fn get_confirmed_tx_channel(&self) -> mpsc::Receiver<Transaction>;
}

/// Create a new consensus engine based on the configuration
pub fn create_consensus_engine(
    config: config::ConsensusConfig,
) -> Result<Box<dyn ConsensusEngine>> {
    match config.engine_type.as_str() {
        "raft" => {
            let engine = raft::RaftConsensusEngine::new(config)?;
            Ok(Box::new(engine))
        }
        _ => Err(anyhow::anyhow!(
            "Unsupported consensus engine type: {}",
            config.engine_type
        )),
    }
}
