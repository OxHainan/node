use anyhow::{anyhow, Result};
use mp_common::types::{Transaction, TransactionResponse, TransactionStatus};
use mp_consensus::ConsensusEngine;
use mp_poc::bls::SignedAggregate;
use mp_poc::PoC;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, info};
use uuid;
use uuid::Uuid;

use crate::config::MempoolConfig;
use crate::TransactionPool;
use mp_common::types::TransactionStatusWithProof;

/// Basic transaction pool implementation
pub struct BasicTransactionPool {
    config: MempoolConfig,
    pending_transactions: Arc<Mutex<VecDeque<Transaction>>>,
    transaction_map: Arc<Mutex<HashMap<Uuid, Transaction>>>,
    transaction_results: Arc<Mutex<HashMap<Uuid, TransactionResponse>>>,
    transaction_proof: Arc<Mutex<HashMap<Uuid, serde_json::Value>>>,
    consensus_engine: Arc<Box<dyn ConsensusEngine>>,
    tx_sender: mpsc::Sender<Transaction>,
    tx_receiver: Mutex<Option<mpsc::Receiver<Transaction>>>,
}

impl BasicTransactionPool {
    /// Create a new basic transaction pool
    pub fn new(config: MempoolConfig, consensus_engine: Box<dyn ConsensusEngine>) -> Result<Self> {
        let (tx_sender, tx_receiver) = mpsc::channel(1000);

        Ok(Self {
            config,
            pending_transactions: Arc::new(Mutex::new(VecDeque::new())),
            transaction_map: Arc::new(Mutex::new(HashMap::new())),
            transaction_results: Arc::new(Mutex::new(HashMap::new())),
            transaction_proof: Arc::new(Mutex::new(HashMap::new())),
            consensus_engine: Arc::new(consensus_engine),
            tx_sender,
            tx_receiver: Mutex::new(Some(tx_receiver)),
        })
    }

    /// Process pending transactions
    async fn process_transactions(&self) {
        info!("Starting transaction processing loop");

        loop {
            // Get a transaction from the queue
            let tx = {
                let mut queue = self.pending_transactions.lock().await;
                queue.pop_front()
            };

            if let Some(transaction) = tx {
                let tx_id = transaction.id;
                debug!("Processing transaction: {}", tx_id);

                // Update transaction status to processing
                {
                    let mut results = self.transaction_results.lock().await;
                    if let Some(response) = results.get_mut(&transaction.id) {
                        response.status = TransactionStatus::Processing;
                    }
                }

                // Submit to consensus engine
                match self
                    .consensus_engine
                    .submit_transaction(transaction.clone())
                    .await
                {
                    Ok(response) => {
                        info!("Transaction submitted for execution: {}", tx_id);
                    }
                    Err(e) => {
                        error!("Failed to submit transaction to consensus: {}", e);

                        // Update transaction status to failed
                        let mut results = self.transaction_results.lock().await;
                        if let Some(response) = results.get_mut(&tx_id) {
                            response.status = TransactionStatus::Error;
                            response.result = Some(
                                serde_json::json!({"error": format!("Failed to submit transaction: {}", e)}),
                            );
                        }

                        // Put it back in the queue for retry
                        let mut queue = self.pending_transactions.lock().await;
                        queue.push_back(transaction);
                    }
                }
            } else {
                // No transactions to process, wait a bit
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }
    }
}

#[async_trait::async_trait]
impl TransactionPool for BasicTransactionPool {
    async fn start(&self) -> Result<()> {
        info!("Starting transaction pool with execute-then-consensus model");

        // 创建交易处理循环
        let pending_transactions = self.pending_transactions.clone();
        let _transaction_map = self.transaction_map.clone();
        let transaction_results = self.transaction_results.clone();
        let tx_sender = self.tx_sender.clone();

        // Start processing thread
        tokio::spawn(async move {
            info!("Transaction pool processing thread started");
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

                // Get a pending transaction
                let tx = {
                    let mut queue = pending_transactions.lock().await;
                    queue.pop_front()
                };

                if let Some(transaction) = tx {
                    let tx_id = transaction.id;
                    info!("Processing pending transaction: {}", tx_id);

                    // Send to transaction channel for execution
                    if let Err(e) = tx_sender.send(transaction.clone()).await {
                        error!("Failed to send transaction to channel: {}", e);
                        continue;
                    }

                    // Update transaction status to processing
                    {
                        let mut results = transaction_results.lock().await;
                        if !results.contains_key(&tx_id) {
                            // Create initial response
                            results.insert(
                                tx_id.clone(),
                                mp_common::types::TransactionResponse {
                                    tx_id: tx_id.clone(),
                                    status: TransactionStatus::Processing,
                                    result: None,
                                },
                            );
                        } else if let Some(response) = results.get_mut(&tx_id) {
                            response.status = TransactionStatus::Processing;
                        }
                    }

                    // Keep the transaction in the map for status tracking
                    info!("Transaction {} sent for execution", tx_id);
                }
            }
        });

        // // Start the API server
        // if let Some(api_address) = &self.config.api_address {
        //     info!("Preparing to start API server on {}", api_address);

        //     match api_address.parse() {
        //         Ok(socket_addr) => {
        //             info!("Successfully parsed API address: {}", socket_addr);

        //             let api_server =
        //                 crate::api::ApiServer::new(Arc::new(self.clone()), socket_addr);

        //             // Spawn a task to start the API server
        //             tokio::spawn(async move {
        //                 info!("Starting API server task");
        //                 match api_server.start().await {
        //                     Ok(_) => info!("API server started successfully"),
        //                     Err(e) => error!("API server error: {}", e),
        //                 }
        //             });

        //             // Give the server a moment to start
        //             tokio::task::yield_now().await;

        //             info!("API server initialization completed");
        //         }
        //         Err(e) => {
        //             error!("Failed to parse API address '{}': {}", api_address, e);
        //             return Err(anyhow::anyhow!(
        //                 "Invalid API address '{}': {}",
        //                 api_address,
        //                 e
        //             ));
        //         }
        //     }
        // } else {
        //     warn!("API server is disabled because api_address is not configured");
        // }

        Ok(())
    }

    fn stop(&self) -> Result<()> {
        info!("Stopping transaction pool");
        // TODO: Implement clean shutdown
        Ok(())
    }

    async fn submit_transaction(&self, transaction: Transaction) -> Result<TransactionResponse> {
        info!("Submitting transaction: {:?}", transaction.id);

        // Add transaction to pending queue
        {
            let mut queue = self.pending_transactions.lock().await;
            queue.push_back(transaction.clone());
        }

        // Store transaction in map
        {
            let mut map = self.transaction_map.lock().await;
            map.insert(transaction.id, transaction.clone());
        }

        // Create initial response
        let response = TransactionResponse {
            tx_id: transaction.id,
            status: TransactionStatus::Pending,
            result: None,
        };

        // Store the initial response
        let mut results = self.transaction_results.lock().await;
        results.insert(transaction.id, response.clone());

        // Also add to transaction_map if not present, to ensure it can be found by get_transaction_status
        let mut tx_map = self.transaction_map.lock().await;
        if !tx_map.contains_key(&transaction.id) {
            // Create a placeholder transaction entry with the ID
            // This ensures the transaction can be found by the client
            // let placeholder_tx = Transaction {
            //     id: transaction.id,
            //     tx_type: mp_common::types::TransactionType::StateChange,
            //     payload: vec![],
            //     timestamp: chrono::Utc::now(),
            //     sender: Some("".to_string()),
            //     log_index: 0,
            // };
            // 直接使用 tx_id 作为键，因为它已经是 String
            tx_map.insert(transaction.id, transaction.clone());
            info!(
                "MEMPOOL - Added placeholder transaction for {}",
                transaction.id
            );
        }

        Ok(response)
    }

    async fn get_pending_tx_channel(&self) -> mpsc::Receiver<Transaction> {
        let mut rx_guard = self.tx_receiver.lock().await;
        rx_guard
            .take()
            .expect("Pending transaction channel already taken")
    }

    async fn get_transaction_status(&self, tx_id: &Uuid) -> Result<TransactionStatusWithProof> {
        // Check if transaction exists in results
        let results = self.transaction_results.lock().await;
        if let Some(response) = results.get(tx_id) {
            info!(
                "MEMPOOL - Found transaction {} in results with status {:?}",
                tx_id, response.status
            );

            // Log if we have a result or not
            if let Some(result) = &response.result {
                info!("MEMPOOL - Transaction has result: {}", result);

                match response.status {
                    TransactionStatus::Pending => return Ok(TransactionStatusWithProof::Pending),
                    TransactionStatus::Processing => {
                        return Ok(TransactionStatusWithProof::Processing)
                    }
                    TransactionStatus::Success => {
                        let poc = self.transaction_proof.lock().await.get(tx_id).cloned();

                        return Ok(TransactionStatusWithProof::Confirmed(
                            response.result.clone().unwrap_or_default(),
                            200,
                            None,
                            poc.map(|p| serde_json::json!(p)),
                        ));
                    }
                    TransactionStatus::Error => {
                        let poc = self.transaction_proof.lock().await.get(tx_id).cloned();

                        return Ok(TransactionStatusWithProof::Failed(
                            response.result.clone().unwrap_or_default(),
                            500,
                            None,
                            poc,
                        ));
                    }
                }
            }
        }

        // Check if transaction exists in pending
        let map = self.transaction_map.lock().await;
        if map.contains_key(tx_id) {
            info!("MEMPOOL - Found transaction {} in pending map", tx_id);
            return Ok(TransactionStatusWithProof::Pending);
        }

        // Transaction not found
        info!("MEMPOOL - Transaction {} not found", tx_id);
        Err(anyhow!("Transaction not found: {}", tx_id))
    }

    /// Update the result of a transaction
    async fn update_transaction_result(
        &self,
        tx_id: &Uuid,
        result: serde_json::Value,
        signed_aggregate: SignedAggregate,
    ) -> Result<()> {
        debug!("Updating result for transaction: {}", tx_id);

        // Log detailed information about the result
        info!(
            "MEMPOOL - Transaction execution result received: {}",
            result
        );
        if let Some(obj) = result.as_object() {
            for (key, value) in obj {
                info!("MEMPOOL - Result field '{}': {}", key, value);
            }
        }

        // Update transaction result
        let mut results = self.transaction_results.lock().await;
        if let Some(response) = results.get_mut(tx_id) {
            // Set status to success
            response.status = TransactionStatus::Success;

            // CRITICAL: Set result EXACTLY as received from executor
            // We must preserve the original structure completely unchanged
            response.result = Some(result.clone());

            info!("MEMPOOL - Updated result for transaction: {}", tx_id);
            // Log what was actually stored to verify it's correctly preserved
            if let Some(stored_result) = &response.result {
                info!("MEMPOOL - Stored result: {}", stored_result);

                // Log each field of the stored result to verify proper preservation
                if let Some(obj) = stored_result.as_object() {
                    for (key, value) in obj {
                        info!("MEMPOOL - Verified field '{}': {}", key, value);
                    }
                }
            }

            let poc: PoC = signed_aggregate.try_into()?;

            self.transaction_proof
                .lock()
                .await
                .insert(*tx_id, serde_json::json!(poc));

            // 在"先执行后共识"模型中，从交易映射中删除，表示处理完成
            let mut map = self.transaction_map.lock().await;
            if map.remove(tx_id).is_some() {
                info!(
                    "MEMPOOL - Transaction {} processing completed and removed from active map",
                    tx_id
                );
            }

            Ok(())
        } else {
            // Transaction not found in results map - create a new result entry
            info!(
                "MEMPOOL - Creating new result entry for transaction: {}",
                tx_id
            );

            let response = TransactionResponse {
                tx_id: *tx_id,
                status: TransactionStatus::Success,
                result: Some(result),
            };

            // Store the result in the transaction_results map
            results.insert(*tx_id, response);

            // Also add to transaction_map if not present, to ensure it can be found by get_transaction_status
            let tx_map = self.transaction_map.lock().await;

            // Check if the transaction ID exists in the map
            if !tx_map.contains_key(tx_id) {
                // Create a placeholder transaction entry with the ID
                // This ensures the transaction can be found by the client
                // let placeholder_tx = Transaction {
                //     id: *tx_id,
                //     tx_type: mp_common::types::TransactionType::StateChange,
                //     payload: vec![],
                //     module_id: None,
                //     timestamp: chrono::Utc::now(),
                //     sender: Some("".to_string()),
                //     log_index: 0,
                // };
                // Add the placeholder transaction to the map
                // tx_map.insert(*tx_id, transaction.clone());
                info!("MEMPOOL - Added placeholder transaction for {}", tx_id);
            }

            Ok(())
        }
    }

    /// Get the result of a transaction
    async fn get_transaction_result(
        &self,
        tx_id: &Uuid,
    ) -> Result<(serde_json::Value, Option<serde_json::Value>)> {
        debug!("Getting result for transaction: {}", tx_id);

        // Check if transaction exists in results
        let results = self.transaction_results.lock().await;
        if let Some(response) = results.get(tx_id) {
            info!(
                "MEMPOOL - Found transaction {} in results with status {:?}",
                tx_id, response.status
            );

            // Return the result if available
            if let Some(result) = &response.result {
                info!("MEMPOOL - Transaction has result: {}", result);
                let poc = self.transaction_proof.lock().await.get(tx_id).cloned();
                return Ok((result.clone(), poc));
            }
        }

        // Transaction not found in results
        info!("MEMPOOL - Transaction {} not found in results", tx_id);
        Ok((serde_json::Value::Null, None))
    }

    async fn get_transaction_proof(&self, tx_id: &Uuid) -> Option<serde_json::Value> {
        self.transaction_proof.lock().await.get(tx_id).cloned()
    }
}

impl Clone for BasicTransactionPool {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            pending_transactions: Arc::clone(&self.pending_transactions),
            transaction_map: Arc::clone(&self.transaction_map),
            transaction_results: Arc::clone(&self.transaction_results),
            consensus_engine: Arc::clone(&self.consensus_engine),
            transaction_proof: Arc::clone(&self.transaction_proof),
            tx_sender: self.tx_sender.clone(),
            tx_receiver: Mutex::new(None), // The receiver can't be cloned
        }
    }
}
