use anyhow::{anyhow, Context, Result};
use clap::Parser;
use config::{Config, File};
use std::path::Path;
use std::sync::Arc;
use tokio::select;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

mod admin;
mod api_key_store;
mod gateway;

use admin::AdminInterface;
use api_key_store::ApiKeyStore;
use gateway::{GatewayConfig, mpRestGateway};

/// Command line arguments
#[derive(Parser, Debug)]
#[clap(author, version, about = "mp RESTful Gateway")]
struct Args {
    /// Path to the configuration file
    #[clap(short, long, default_value = "config.toml")]
    config: String,

    /// Log level
    #[clap(short, long, default_value = "info")]
    log_level: String,
}

/// Gateway configuration from TOML file
#[derive(Debug, serde::Deserialize)]
struct AppConfig {
    /// REST gateway settings
    gateway: GatewayConfig,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let args = Args::parse();

    // Initialize logging
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&args.log_level));

    tracing_subscriber::fmt().with_env_filter(filter).init();

    info!("Starting mp RESTful Gateway");

    // Load configuration
    let config_path = Path::new(&args.config);
    let config = load_config(config_path).context(format!(
        "Failed to load config from {}",
        config_path.display()
    ))?;

    info!("Configuration loaded from {}", config_path.display());
    info!("Node URL: {}", config.gateway.node_url);
    info!("Gateway address: {}", config.gateway.gateway_bind_address);
    info!("Admin address: {}", config.gateway.admin_bind_address);

    // Create API key store
    let api_key_store = ApiKeyStore::new(&config.gateway.key_store_path)
        .await
        .context("Failed to initialize API key store")?;

    // Create admin interface
    let admin = AdminInterface::new(api_key_store.clone());

    // Create gateway
    let gateway = mpRestGateway::new(config.gateway.clone(), api_key_store);

    // Start both services concurrently
    select! {
        result = admin.start(&config.gateway.admin_bind_address) => {
            if let Err(e) = result {
                error!("Admin interface error: {}", e);
                return Err(anyhow!("Admin interface error: {}", e));
            }
        },
        result = gateway.start() => {
            if let Err(e) = result {
                error!("Gateway error: {}", e);
                return Err(anyhow!("Gateway error: {}", e));
            }
        }
    }

    Ok(())
}

/// Load the gateway configuration from a TOML file
fn load_config(config_path: &Path) -> Result<AppConfig> {
    // Load the configuration file using the config crate
    let config_file = Config::builder()
        .add_source(File::from(config_path))
        .build()
        .context(format!(
            "Failed to load config file: {}",
            config_path.display()
        ))?;

    // Extract gateway section
    let app_config: AppConfig = config_file
        .try_deserialize()
        .context("Failed to parse config file")?;

    Ok(app_config)
}
