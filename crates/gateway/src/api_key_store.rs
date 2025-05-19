use anyhow::{anyhow, Result};
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task;
use tracing::info;

/// Store for API keys and their associated blockchain addresses
pub struct ApiKeyStore {
    /// Path to the SQLite database
    db_path: String,
    /// Cache of API key -> (address, nonce) mappings
    cache: RwLock<HashMap<String, (String, u64)>>,
}

impl ApiKeyStore {
    /// Create a new API key store
    pub async fn new(db_path: &str) -> Result<Arc<Self>> {
        let store = Self {
            db_path: db_path.to_string(),
            cache: RwLock::new(HashMap::new()),
        };

        // Initialize the database
        store.init_db().await?;

        Ok(Arc::new(store))
    }

    /// Initialize the database
    async fn init_db(&self) -> Result<()> {
        let db_path = self.db_path.clone();

        // 使用 spawn_blocking 在专用线程上执行 SQLite 操作
        task::spawn_blocking(move || -> Result<()> {
            // Create the database file if it doesn't exist
            let conn = Connection::open(&db_path)?;

            // Create the api_keys table if it doesn't exist
            conn.execute(
                "CREATE TABLE IF NOT EXISTS api_keys (
                    key TEXT PRIMARY KEY,
                    address TEXT NOT NULL,
                    nonce INTEGER NOT NULL DEFAULT 0
                )",
                [],
            )?;

            Ok(())
        })
        .await??;

        info!("API key database initialized at {}", self.db_path);
        Ok(())
    }

    /// Get the blockchain address and nonce for an API key
    pub async fn get_address_and_nonce(&self, api_key: &str) -> Result<(String, u64)> {
        // Check the cache first
        {
            let cache = self.cache.read().await;
            if let Some((address, nonce)) = cache.get(api_key) {
                return Ok((address.clone(), *nonce));
            }
        }

        let api_key_clone = api_key.to_string(); // 创建一个克隆用于闭包
        let db_path = self.db_path.clone();

        // 使用 spawn_blocking 在专用线程上执行 SQLite 操作
        let result = task::spawn_blocking(move || -> Result<Option<(String, u64)>> {
            let conn = Connection::open(&db_path)?;

            let mut stmt = conn.prepare("SELECT address, nonce FROM api_keys WHERE key = ?")?;
            let mut rows = stmt.query(params![api_key_clone])?;

            if let Some(row) = rows.next()? {
                let address: String = row.get(0)?;
                let nonce: u64 = row.get(1)?;
                Ok(Some((address, nonce)))
            } else {
                Ok(None)
            }
        })
        .await??;

        if let Some((address, nonce)) = result {
            // Update the cache
            let mut cache = self.cache.write().await;
            cache.insert(api_key.to_string(), (address.clone(), nonce));

            Ok((address, nonce))
        } else {
            Err(anyhow!("API key not found"))
        }
    }

    /// Add a new API key
    pub async fn add_api_key(&self, api_key: &str, address: &str) -> Result<()> {
        let api_key_clone = api_key.to_string(); // 创建克隆用于闭包
        let address_clone = address.to_string(); // 创建克隆用于闭包
        let db_path = self.db_path.clone();

        // 使用 spawn_blocking 在专用线程上执行 SQLite 操作
        task::spawn_blocking(move || -> Result<()> {
            let conn = Connection::open(&db_path)?;

            conn.execute(
                "INSERT OR REPLACE INTO api_keys (key, address, nonce) VALUES (?, ?, 0)",
                params![api_key_clone, address_clone],
            )?;

            Ok(())
        })
        .await??;

        // Update the cache
        let mut cache = self.cache.write().await;
        cache.insert(api_key.to_string(), (address.to_string(), 0));

        info!("Added API key for address: {}", address);
        Ok(())
    }

    /// Delete an API key
    pub async fn delete_api_key(&self, api_key: &str) -> Result<()> {
        let api_key_clone = api_key.to_string(); // 创建克隆用于闭包
        let db_path = self.db_path.clone();

        // 使用 spawn_blocking 在专用线程上执行 SQLite 操作
        task::spawn_blocking(move || -> Result<()> {
            let conn = Connection::open(&db_path)?;

            conn.execute("DELETE FROM api_keys WHERE key = ?", params![api_key_clone])?;

            Ok(())
        })
        .await??;

        // Update the cache
        let mut cache = self.cache.write().await;
        cache.remove(api_key);

        info!("Deleted API key");
        Ok(())
    }

    /// List all API keys
    pub async fn list_api_keys(&self) -> Result<Vec<(String, String, u64)>> {
        let db_path = self.db_path.clone();

        // 使用 spawn_blocking 在专用线程上执行 SQLite 操作
        let keys = task::spawn_blocking(move || -> Result<Vec<(String, String, u64)>> {
            let conn = Connection::open(&db_path)?;

            let mut stmt = conn.prepare("SELECT key, address, nonce FROM api_keys")?;
            let rows = stmt.query_map([], |row| {
                let key: String = row.get(0)?;
                let address: String = row.get(1)?;
                let nonce: u64 = row.get(2)?;
                Ok((key, address, nonce))
            })?;

            let mut keys = Vec::new();
            for row in rows {
                keys.push(row?);
            }

            Ok(keys)
        })
        .await??;

        // Update the cache
        let mut cache = self.cache.write().await;
        for (key, address, nonce) in &keys {
            cache.insert(key.clone(), (address.clone(), *nonce));
        }

        Ok(keys)
    }

    /// Increment the nonce for an API key
    pub async fn increment_nonce(&self, api_key: &str) -> Result<u64> {
        let api_key_clone = api_key.to_string(); // 创建克隆用于闭包
        let db_path = self.db_path.clone();

        // 使用 spawn_blocking 在专用线程上执行 SQLite 操作
        let new_nonce = task::spawn_blocking(move || -> Result<u64> {
            let conn = Connection::open(&db_path)?;

            // Get the current nonce
            let mut stmt = conn.prepare("SELECT nonce FROM api_keys WHERE key = ?")?;
            let mut rows = stmt.query(params![api_key_clone])?;

            let current_nonce: u64 = if let Some(row) = rows.next()? {
                row.get(0)? // 明确指定类型为 u64
            } else {
                return Err(anyhow!("API key not found"));
            };

            // Increment the nonce
            let new_nonce = current_nonce + 1;

            conn.execute(
                "UPDATE api_keys SET nonce = ? WHERE key = ?",
                params![new_nonce, api_key_clone],
            )?;

            Ok(new_nonce)
        })
        .await??;

        // Update the cache
        {
            let mut cache = self.cache.write().await;
            if let Some((address, _)) = cache.get(api_key).cloned() {
                cache.insert(api_key.to_string(), (address, new_nonce));
            }
        }

        Ok(new_nonce)
    }

    /// Alias for delete_api_key to maintain compatibility
    pub async fn remove_api_key(&self, api_key: &str) -> Result<()> {
        self.delete_api_key(api_key).await
    }
}
