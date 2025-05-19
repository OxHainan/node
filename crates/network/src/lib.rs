//! Network module for mp blockchain

pub mod config;
mod discovery;
mod protocol;

use anyhow::Result;
use mp_common::types::Transaction;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use discovery::DiscoveryService;
use protocol::ProtocolHandler;

/// Network interface
pub trait Network: Send + Sync {
    /// Start the network
    fn start(&self) -> Result<()>;

    /// Stop the network
    fn stop(&self) -> Result<()>;

    /// Broadcast a transaction to the network
    fn broadcast_transaction(&self, transaction: Transaction) -> Result<()>;

    /// Get a channel for received transactions
    fn get_received_tx_channel(&self) -> mpsc::Receiver<Transaction>;

    /// Clone this network instance
    fn clone(&self) -> Arc<dyn Network>;
}

/// Create a new network based on the configuration
pub fn create_network(config: config::NetworkConfig) -> Result<Arc<dyn Network>> {
    info!("Creating libp2p network service");
    Ok(Arc::new(Libp2pNetwork::new(config)?))
}

/// Libp2p-based network implementation
struct Libp2pNetwork {
    config: config::NetworkConfig,
    discovery: Arc<Mutex<DiscoveryService>>,
    protocol: Arc<Mutex<ProtocolHandler>>,
    running: Arc<Mutex<bool>>,
    tx_sender: mpsc::Sender<Transaction>,
    tx_receiver: tokio::sync::Mutex<Option<mpsc::Receiver<Transaction>>>,
}

impl Libp2pNetwork {
    /// Create a new libp2p network
    fn new(config: config::NetworkConfig) -> Result<Self> {
        let (tx_sender, tx_receiver) = mpsc::channel(1000);

        Ok(Self {
            config: config.clone(),
            discovery: Arc::new(Mutex::new(DiscoveryService::new(config.clone()))),
            protocol: Arc::new(Mutex::new(ProtocolHandler::new(config))),
            running: Arc::new(Mutex::new(false)),
            tx_sender,
            tx_receiver: tokio::sync::Mutex::new(Some(tx_receiver)),
        })
    }
}

impl Network for Libp2pNetwork {
    fn start(&self) -> Result<()> {
        info!(
            "Starting libp2p network service on {}",
            self.config.listen_address
        );

        let mut running = self.running.lock().unwrap();
        if *running {
            warn!("Network service already running");
            return Ok(());
        }

        // Start discovery service
        if let Err(e) = self.discovery.lock().unwrap().start() {
            error!("Failed to start discovery service: {}", e);
            return Err(anyhow::anyhow!("Failed to start discovery service"));
        }

        // Start protocol handler
        if let Err(e) = self.protocol.lock().unwrap().start() {
            error!("Failed to start protocol handler: {}", e);
            return Err(anyhow::anyhow!("Failed to start protocol handler"));
        }

        *running = true;
        info!("Network service started");

        Ok(())
    }

    fn stop(&self) -> Result<()> {
        info!("Stopping libp2p network service");

        let mut running = self.running.lock().unwrap();
        if !*running {
            warn!("Network service not running");
            return Ok(());
        }

        // Stop protocol handler
        if let Err(e) = self.protocol.lock().unwrap().stop() {
            error!("Failed to stop protocol handler: {}", e);
        }

        // Stop discovery service
        if let Err(e) = self.discovery.lock().unwrap().stop() {
            error!("Failed to stop discovery service: {}", e);
        }

        *running = false;
        info!("Network service stopped");

        Ok(())
    }

    fn broadcast_transaction(&self, transaction: Transaction) -> Result<()> {
        debug!("Broadcasting transaction: {:?}", transaction.id);

        let running = self.running.lock().unwrap();
        if !*running {
            return Err(anyhow::anyhow!("Network service not running"));
        }

        // Forward to protocol handler
        self.protocol
            .lock()
            .unwrap()
            .broadcast_transaction(transaction)
    }

    fn get_received_tx_channel(&self) -> mpsc::Receiver<Transaction> {
        let mut rx_guard = self.tx_receiver.blocking_lock();
        rx_guard
            .take()
            .expect("Received transaction channel already taken")
    }

    fn clone(&self) -> Arc<dyn Network> {
        Arc::new(Libp2pNetwork {
            config: self.config.clone(),
            discovery: self.discovery.clone(),
            protocol: self.protocol.clone(),
            running: self.running.clone(),
            tx_sender: self.tx_sender.clone(),
            tx_receiver: tokio::sync::Mutex::new(None),
        })
    }
}
