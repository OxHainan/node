use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

/// Store for API keys and their associated blockchain addresses
#[derive(Debug)]
pub struct ApiKeyStore {
    /// Path to the storage file
    path: String,
    /// API key to account mapping
    accounts: Arc<Mutex<HashMap<String, AccountInfo>>>,
}

/// Account information associated with an API key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountInfo {
    /// Blockchain address associated with this API key
    pub address: String,
    /// Current nonce for this address
    pub nonce: u64,
    /// Name of the API key (optional)
    pub name: Option<String>,
    /// Time when the API key was created
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl ApiKeyStore {
    /// Create a new API key store
    pub async fn new(path: &str) -> Result<Arc<Self>> {
        let store = Self {
            path: path.to_string(),
            accounts: Arc::new(Mutex::new(HashMap::new())),
        };

        // Load accounts from file if it exists
        if Path::new(path).exists() {
            let content = tokio::fs::read_to_string(path).await?;
            let accounts: HashMap<String, AccountInfo> = serde_json::from_str(&content)?;
            *(store.accounts.lock().await) = accounts;
        } else {
            // Create directory if it doesn't exist
            if let Some(parent) = Path::new(path).parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            // Initialize empty store and save it
            store.save().await?;
        }

        Ok(Arc::new(store))
    }

    /// Get address and nonce for an API key
    pub async fn get_address_and_nonce(&self, api_key: &str) -> Result<(String, u64)> {
        let accounts = self.accounts.lock().await;

        let account_info = accounts
            .get(api_key)
            .ok_or_else(|| anyhow!("Invalid API key"))?;

        Ok((account_info.address.clone(), account_info.nonce))
    }

    /// Increment the nonce for an API key
    pub async fn increment_nonce(&self, api_key: &str) -> Result<()> {
        let mut accounts = self.accounts.lock().await;

        let account_info = accounts
            .get_mut(api_key)
            .ok_or_else(|| anyhow!("Invalid API key"))?;

        account_info.nonce += 1;

        // Save updated accounts to file
        drop(accounts); // Release lock before calling save
        self.save().await?;

        Ok(())
    }

    /// Generate a new API key and associate it with an address
    pub async fn generate_key(&self, name: Option<String>, address: &str) -> Result<String> {
        let mut accounts = self.accounts.lock().await;

        // Generate API key (UUID)
        let api_key = Uuid::new_v4().to_string();

        // Create account info
        let account_info = AccountInfo {
            address: address.to_string(),
            nonce: 0,
            name,
            created_at: chrono::Utc::now(),
        };

        // Add to accounts
        accounts.insert(api_key.clone(), account_info);

        // Save updated accounts to file
        drop(accounts); // Release lock before calling save
        self.save().await?;

        Ok(api_key)
    }

    /// Revoke an API key
    pub async fn revoke_key(&self, api_key: &str) -> Result<()> {
        let mut accounts = self.accounts.lock().await;

        // Remove API key from accounts
        accounts
            .remove(api_key)
            .ok_or_else(|| anyhow!("Invalid API key"))?;

        // Save updated accounts to file
        drop(accounts); // Release lock before calling save
        self.save().await?;

        Ok(())
    }

    /// Get all API keys and their associated information
    pub async fn get_all_keys(&self) -> Result<HashMap<String, AccountInfo>> {
        let accounts = self.accounts.lock().await;
        Ok(accounts.clone())
    }

    /// Save current state to file
    async fn save(&self) -> Result<()> {
        let accounts = self.accounts.lock().await;
        let content = serde_json::to_string_pretty(&*accounts)?;
        tokio::fs::write(&self.path, content).await?;
        Ok(())
    }
}
