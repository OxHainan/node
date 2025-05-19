use anyhow::Result;
use mp_common::types::{Transaction, TransactionResponse};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, RwLock};
use tokio::time::{self, Duration};
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::config::{ConsensusConfig, NodeInfo};
use crate::ConsensusEngine;

/// Simplified Raft node states
#[derive(Debug, Clone, PartialEq, Eq)]
enum NodeState {
    Follower,
    Candidate,
    Leader,
}

/// A simplified implementation of the Raft consensus algorithm
pub struct RaftConsensusEngine {
    /// Node ID
    node_id: u64,
    /// Current node state
    state: Arc<RwLock<NodeState>>,
    /// List of all nodes in the cluster
    nodes: Vec<NodeInfo>,
    /// Current term
    current_term: Arc<RwLock<u64>>,
    /// Current leader ID
    leader_id: Arc<RwLock<Option<u64>>>,
    /// Log entries (simplified as a queue of transactions)
    log: Arc<Mutex<VecDeque<Transaction>>>,
    /// Last applied log index
    last_applied: Arc<RwLock<u64>>,
    /// Commit index
    commit_index: Arc<RwLock<u64>>,
    /// Channel for sending confirmed transactions to subscribers
    confirmed_tx_sender: mpsc::Sender<Transaction>,
    /// Channel for receiving confirmed transactions
    confirmed_tx_receiver: Arc<Mutex<Option<mpsc::Receiver<Transaction>>>>,
    /// Heartbeat interval in milliseconds
    heartbeat_interval: u64,
    /// Election timeout range in milliseconds
    election_timeout_min: u64,
    election_timeout_max: u64,
    /// Transaction responses (transaction ID -> response)
    responses: Arc<Mutex<HashMap<Uuid, TransactionResponse>>>,
    /// Flag to indicate if the engine is running
    running: Arc<RwLock<bool>>,
    /// Store for transaction results to be propagated in heartbeats
    transaction_results: Arc<RwLock<Vec<TransactionResponse>>>,
}

impl RaftConsensusEngine {
    /// Create a new Raft consensus engine
    pub fn new(config: ConsensusConfig) -> Result<Self> {
        let raft_config = config
            .raft
            .ok_or_else(|| anyhow::anyhow!("Raft configuration is required"))?;

        // Create channels for confirmed transactions
        let (confirmed_tx_sender, confirmed_tx_receiver) = mpsc::channel(1000);

        Ok(Self {
            node_id: config.node_id,
            state: Arc::new(RwLock::new(NodeState::Follower)),
            nodes: config.nodes,
            current_term: Arc::new(RwLock::new(0)),
            leader_id: Arc::new(RwLock::new(None)),
            log: Arc::new(Mutex::new(VecDeque::new())),
            last_applied: Arc::new(RwLock::new(0)),
            commit_index: Arc::new(RwLock::new(0)),
            confirmed_tx_sender,
            confirmed_tx_receiver: Arc::new(Mutex::new(Some(confirmed_tx_receiver))),
            heartbeat_interval: raft_config.heartbeat_interval,
            election_timeout_min: raft_config.election_timeout_min,
            election_timeout_max: raft_config.election_timeout_max,
            responses: Arc::new(Mutex::new(HashMap::new())),
            running: Arc::new(RwLock::new(false)),
            transaction_results: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Start the heartbeat process (leader only)
    async fn start_heartbeat(&self) {
        // Clone all necessary values to avoid self reference in tokio::spawn
        let running = self.running.clone();
        let state = self.state.clone();
        let heartbeat_interval = self.heartbeat_interval;
        let transaction_results = self.transaction_results.clone();

        // Clone the propagate_transaction_results method by creating a channel
        let (tx_sender, mut tx_receiver) = mpsc::channel::<Vec<TransactionResponse>>(100);

        // Spawn a task to receive results from heartbeat and propagate them
        let propagate_task = {
            let self_clone = self.clone();
            tokio::spawn(async move {
                while let Some(results) = tx_receiver.recv().await {
                    if let Err(e) = self_clone.propagate_transaction_results(results).await {
                        error!("Failed to propagate transaction results: {}", e);
                    }
                }
            })
        };

        tokio::spawn(async move {
            let interval_duration = Duration::from_millis(heartbeat_interval);
            let mut interval = time::interval(interval_duration);

            while *running.read().await {
                interval.tick().await;

                if *state.read().await == NodeState::Leader {
                    debug!("Sending heartbeat to followers");

                    let results = transaction_results.read().await;
                    if !results.is_empty() {
                        debug!(
                            "Including {} transaction results in heartbeat",
                            results.len()
                        );
                    }

                    // Send results to propagate task instead of calling directly
                    if !results.is_empty() {
                        if let Err(e) = tx_sender.send(results.clone()).await {
                            error!("Failed to send transaction results for propagation: {}", e);
                        }
                    }

                    transaction_results.write().await.clear();
                }
            }

            // Drop sender when done to terminate propagate task
            drop(tx_sender);
        });
    }

    /// Propagate transaction results to followers
    async fn propagate_transaction_results(&self, results: Vec<TransactionResponse>) -> Result<()> {
        // In a real implementation, we would send AppendEntries RPCs to all followers
        // with the transaction results included
        debug!(
            "Propagating {} transaction results to followers",
            results.len()
        );

        // Simulate propagation delay
        time::sleep(Duration::from_millis(10)).await;

        Ok(())
    }

    /// Start the election timeout process (follower/candidate only)
    async fn start_election_timeout(&self) {
        let running = self.running.clone();
        let state = self.state.clone();
        let current_term = self.current_term.clone();
        let node_id = self.node_id;
        let leader_id = self.leader_id.clone();
        let min_timeout = self.election_timeout_min;
        let max_timeout = self.election_timeout_max;

        tokio::spawn(async move {
            while *running.read().await {
                // Only start election timeout if we're a follower or candidate
                let current_state = state.read().await.clone();
                if current_state == NodeState::Leader {
                    time::sleep(Duration::from_millis(100)).await;
                    continue;
                }

                // Random timeout between min and max
                let timeout = rand::random::<u64>() % (max_timeout - min_timeout) + min_timeout;
                let timeout_duration = Duration::from_millis(timeout);

                // Wait for timeout
                time::sleep(timeout_duration).await;

                // Check if we're still a follower/candidate and should start an election
                let current_state = state.read().await.clone();
                if (current_state == NodeState::Follower || current_state == NodeState::Candidate)
                    && *running.read().await
                {
                    // Get current term
                    let current_term_value = *current_term.read().await;
                    // Start election
                    info!("Starting election for term {}", current_term_value + 1);

                    // Increment term
                    let mut term = current_term.write().await;
                    *term += 1;

                    // Become candidate
                    *state.write().await = NodeState::Candidate;

                    // Vote for self
                    // In a real implementation, we would send RequestVote RPCs to all other nodes

                    // For simplicity in this demo, always become leader after a timeout
                    // In a real implementation, we would only become leader if we received votes from a majority
                    time::sleep(Duration::from_millis(100)).await;
                    if *state.read().await == NodeState::Candidate {
                        info!("Node {} became leader for term {}", node_id, *term);
                        *state.write().await = NodeState::Leader;
                        *leader_id.write().await = Some(node_id);
                    }
                }
            }
        });
    }

    /// Process a transaction (leader only)
    async fn process_transaction(&self, tx: Transaction) -> Result<TransactionResponse> {
        // Create initial response
        let response = TransactionResponse::success(tx.id);

        // Store response in responses map
        {
            let mut responses = self.responses.lock().unwrap();
            responses.insert(tx.id, response.clone());
        }

        // Add transaction to transaction_results for propagation in heartbeats
        {
            let mut transaction_results = self.transaction_results.write().await;
            transaction_results.push(response.clone());
        }

        // CRITICAL FIX: Add transaction to log for processing
        {
            let mut log = self.log.lock().unwrap();
            log.push_back(tx);

            // Update commit index to ensure this transaction gets processed
            let mut commit_index = self.commit_index.write().await;
            *commit_index += 1;
        }

        Ok(response)
    }

    /// Apply a committed transaction
    async fn apply_transaction(&self, transaction: Transaction) -> Result<()> {
        // Update last applied index
        {
            let mut last_applied = self.last_applied.write().await;
            *last_applied = transaction.log_index;
        }

        // Send transaction to confirmed channel
        if let Err(e) = self.confirmed_tx_sender.send(transaction).await {
            error!("Failed to send confirmed transaction: {}", e);
        }

        Ok(())
    }

    /// Start the log applier process
    async fn start_log_applier(&self) {
        let running = self.running.clone();
        let log = self.log.clone();
        let last_applied = self.last_applied.clone();
        let commit_index = self.commit_index.clone();
        let confirmed_tx_sender = self.confirmed_tx_sender.clone();

        tokio::spawn(async move {
            while *running.read().await {
                // Check if there are committed entries that haven't been applied
                let last_applied_value = *last_applied.read().await;
                let commit_index_value = *commit_index.read().await;

                if last_applied_value < commit_index_value {
                    // Apply all committed entries
                    for i in (last_applied_value + 1)..=commit_index_value {
                        // Get the transaction from the log
                        let mut tx_clone = {
                            let log_guard = log.lock().unwrap();
                            if let Some(tx) = log_guard.get((i as usize).saturating_sub(1)) {
                                tx.clone()
                            } else {
                                continue;
                            }
                        };

                        // CRITICAL FIX: Set log_index before sending to confirmed channel
                        tx_clone.log_index = i;

                        // Update last applied index
                        *last_applied.write().await = i;

                        // Send transaction to confirmed channel
                        info!("Sending confirmed transaction to executor: {}", tx_clone.id);
                        if let Err(e) = confirmed_tx_sender.send(tx_clone).await {
                            error!("Failed to send confirmed transaction: {}", e);
                        }
                    }
                }

                // Sleep for a short time
                time::sleep(Duration::from_millis(10)).await;
            }
        });
    }
}

#[async_trait::async_trait]
impl ConsensusEngine for RaftConsensusEngine {
    /// Start the consensus engine
    async fn start(&mut self) -> Result<()> {
        info!("Starting Raft consensus engine (node ID: {})", self.node_id);

        // Set running flag
        *self.running.write().await = true;

        // Start as follower
        *self.state.write().await = NodeState::Follower;

        // Start heartbeat process
        self.start_heartbeat().await;

        // Start election timeout process
        self.start_election_timeout().await;

        // Start log applier process
        self.start_log_applier().await;

        info!("Raft consensus engine started");
        Ok(())
    }

    /// Stop the consensus engine
    async fn stop(&mut self) -> Result<()> {
        info!("Stopping Raft consensus engine");

        // Set running flag to false
        *self.running.write().await = false;

        // Wait for a short time to allow tasks to finish
        time::sleep(Duration::from_millis(100)).await;

        info!("Raft consensus engine stopped");
        Ok(())
    }

    /// Submit a transaction to the consensus engine
    async fn submit_transaction(&self, transaction: Transaction) -> Result<TransactionResponse> {
        debug!(
            "Submitting transaction to Raft consensus: {:?}",
            transaction.id
        );

        // Process the transaction - 完全重写，避免跨 .await 使用 MutexGuard
        let response = TransactionResponse::success(transaction.id);

        // Store response in responses map
        {
            let mut responses = self.responses.lock().unwrap();
            responses.insert(transaction.id, response.clone());
        }

        // Add transaction to transaction_results for propagation in heartbeats
        {
            let mut transaction_results = self.transaction_results.write().await;
            transaction_results.push(response.clone());
        }

        // CRITICAL FIX: Add transaction to log for processing
        {
            // 在这个作用域内使用 MutexGuard，并且不跨越 .await
            let mut log = self.log.lock().unwrap();
            log.push_back(transaction);
        }

        // Update commit index to ensure this transaction gets processed
        {
            let mut commit_index = self.commit_index.write().await;
            *commit_index += 1;
        }

        Ok(response)
    }

    /// Get a channel for confirmed transactions
    async fn get_confirmed_tx_channel(&self) -> mpsc::Receiver<Transaction> {
        // 对 Mutex 使用 lock() 而不是 await
        let mut guard = self.confirmed_tx_receiver.lock().unwrap();
        if let Some(rx) = guard.take() {
            rx
        } else {
            // Create a dummy channel if the original one has been taken
            let (_, rx) = mpsc::channel(1);
            rx
        }
    }
}

impl Clone for RaftConsensusEngine {
    fn clone(&self) -> Self {
        // Clone the basic fields
        let (new_tx_sender, mut new_tx_receiver) = mpsc::channel(1000);

        // Clone the original sender's channel
        let original_sender = self.confirmed_tx_sender.clone();

        // 创建一个新通道而不是尝试克隆Receiver，因为Receiver不实现Clone
        // 让我们创建一个新通道，并转发消息
        let new_sender = new_tx_sender.clone();
        tokio::spawn(async move {
            while let Some(tx) = new_tx_receiver.recv().await {
                if let Err(e) = original_sender.send(tx).await {
                    error!("Failed to forward transaction from clone: {}", e);
                    break;
                }
            }
        });

        Self {
            node_id: self.node_id,
            state: self.state.clone(),
            nodes: self.nodes.clone(),
            current_term: self.current_term.clone(),
            leader_id: self.leader_id.clone(),
            log: self.log.clone(),
            last_applied: self.last_applied.clone(),
            commit_index: self.commit_index.clone(),
            confirmed_tx_sender: new_sender,
            confirmed_tx_receiver: Arc::new(Mutex::new(None)),
            heartbeat_interval: self.heartbeat_interval,
            election_timeout_min: self.election_timeout_min,
            election_timeout_max: self.election_timeout_max,
            responses: self.responses.clone(),
            running: self.running.clone(),
            transaction_results: self.transaction_results.clone(),
        }
    }
}
