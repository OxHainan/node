use anyhow::Result;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, error, info};

use crate::core::{ExecutionEngine, ExecutionRequest, ExecutionResult};

/// Cross-process communication bridge for execution
pub struct ExecutionBridge {
    /// Execution engine instance
    engine: Arc<dyn ExecutionEngine>,
    /// Channel for sending execution requests
    request_tx: mpsc::Sender<ExecutionRequest>,
    /// Channel for receiving execution requests
    request_rx: mpsc::Receiver<ExecutionRequest>,
    queue_size: usize,
    /// Worker handles
    workers: Vec<JoinHandle<()>>,
}

impl ExecutionBridge {
    /// Create a new execution bridge
    pub fn new(engine: Arc<dyn ExecutionEngine>, worker_count: usize, queue_size: usize) -> Self {
        let (request_tx, request_rx) = mpsc::channel(queue_size);

        Self {
            engine,
            request_tx,
            request_rx,
            queue_size,
            workers: Vec::with_capacity(worker_count),
        }
    }

    /// Start the execution bridge
    pub async fn start(&mut self, worker_count: usize) -> Result<mpsc::Receiver<ExecutionResult>> {
        info!("Starting execution bridge with {} workers", worker_count);

        // Create result channel
        let (result_tx, result_rx) = mpsc::channel(self.queue_size);

        // Create multiple channels for worker distribution
        let mut worker_channels = Vec::with_capacity(worker_count);
        for i in 0..worker_count {
            let (tx, rx) = mpsc::channel::<ExecutionRequest>(100);
            info!("Created worker channel {} with capacity 100", i);
            worker_channels.push((tx, rx));
        }

        // Forward messages from the main channel to workers in a round-robin fashion
        let worker_senders = worker_channels
            .iter()
            .map(|(tx, _)| tx.clone())
            .collect::<Vec<_>>();
        let mut main_rx =
            std::mem::replace(&mut self.request_rx, mpsc::channel::<ExecutionRequest>(1).1);

        // Spawn a task that distributes incoming requests across workers
        tokio::spawn(async move {
            let mut current_worker = 0;
            while let Some(req) = main_rx.recv().await {
                info!(
                    "Distributing request for module {:?} to worker {}",
                    req.transaction_type, current_worker
                );
                // Round-robin distribution
                if let Err(e) = worker_senders[current_worker].send(req).await {
                    error!(
                        "Failed to forward execution request to worker {}: {}",
                        current_worker, e
                    );
                }
                current_worker = (current_worker + 1) % worker_senders.len();
            }
            info!("Main request channel closed, distributor task ending");
        });

        // Start worker tasks
        for i in 0..worker_count {
            let engine = self.engine.clone();
            let result_tx = result_tx.clone();
            // Take ownership of the receiver - Receivers can't be cloned
            let mut worker_rx = std::mem::replace(
                &mut worker_channels[i].1,
                mpsc::channel::<ExecutionRequest>(1).1,
            );

            let handle = tokio::spawn(async move {
                info!("Worker {} started", i);

                while let Some(mut request) = worker_rx.recv().await {
                    debug!(
                        "Worker {} processing request for module {:?}",
                        i, request.transaction_type
                    );

                    // Log more details about the request
                    info!(
                        "Worker {} processing tx {} for module {:?}",
                        i, request.tx_hash, request.transaction_type
                    );

                    // 执行请求，使用可变引用，以保留 result_sender 用于主动通知
                    match engine.execute(&mut request).await {
                        Ok(result) => {
                            // Add debug logging to print the execution result
                            info!(
                                "Worker {} execution result for tx {}: {}",
                                i,
                                request.tx_hash,
                                serde_json::to_string_pretty(&result.output).unwrap_or_default()
                            );

                            if let Err(e) = result_tx.send(result).await {
                                error!("Failed to send execution result: {}", e);
                            }
                        }
                        Err(e) => {
                            error!(
                                "Worker {} execution error for tx {}: {}",
                                i, request.tx_hash, e
                            );
                        }
                    }
                }

                info!("Worker {} channel closed, worker stopping", i);
            });

            self.workers.push(handle);
        }

        Ok(result_rx)
    }

    /// Get a sender for execution requests
    pub fn get_request_sender(&self) -> mpsc::Sender<ExecutionRequest> {
        self.request_tx.clone()
    }

    /// Stop the execution bridge
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping execution bridge");

        // Dropping channels will cause workers to exit
        // We could implement a more graceful shutdown if needed

        Ok(())
    }
}
