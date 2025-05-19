use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Module container configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleContainerConfig {
    /// Module ID
    pub module_id: String,

    /// Container image to use for this module
    pub container_image: Option<String>,

    /// Port to expose for this module
    pub port: u16,

    /// Environment variables for this module
    pub env_vars: HashMap<String, String>,
}

/// Container mode
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContainerMode {
    #[default]
    Simulated,
    #[serde(rename = "cvm")]
    CVM,
}

/// Container environment configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerConfig {
    /// Container mode
    #[serde(default)]
    pub container_mode: ContainerMode,

    // Docker connection is now handled by Docker::connect_with_local_defaults()
    /// Teepod API address
    #[serde(default = "default_teepod_host")]
    pub teepod_host: String,

    /// Tappd API address
    pub tappd_host: Option<String>,

    /// Maximum number of concurrent containers
    #[serde(default = "default_max_containers")]
    pub max_containers: usize,

    /// Container timeout in seconds
    #[serde(default = "default_container_timeout")]
    pub container_timeout: u64,

    /// Default container image
    #[serde(default = "default_container_image")]
    pub container_image: String,

    /// Network mode for containers
    #[serde(default = "default_network_mode")]
    pub network_mode: String,

    /// Base port for container mapping
    #[serde(default = "default_base_port")]
    pub base_port: u16,

    /// Module-specific container configurations
    #[serde(default)]
    pub module_configs: Vec<ModuleContainerConfig>,

    /// Static container mappings (module_id -> address)
    #[serde(default)]
    pub static_container_mappings: HashMap<String, String>,
}

// Default values for configuration
fn default_teepod_host() -> String {
    "http://127.0.0.1:33001".to_string()
}
pub fn default_tappd_host() -> String {
    "http://127.0.0.1:8090".to_string()
}
fn default_max_containers() -> usize {
    10
}
fn default_container_timeout() -> u64 {
    30
}
fn default_container_image() -> String {
    "mp/executor:latest".to_string()
}
fn default_network_mode() -> String {
    "host".to_string()
}
fn default_base_port() -> u16 {
    3000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config() {
        let mode = ContainerMode::Simulated;
        assert_eq!(mode, ContainerMode::Simulated);
        println!("mode: {}", serde_json::to_string_pretty(&mode).unwrap());

        let mode = ContainerMode::CVM;
        assert_eq!(mode, ContainerMode::CVM);
        println!("mode: {}", serde_json::to_string_pretty(&mode).unwrap());
    }
}
