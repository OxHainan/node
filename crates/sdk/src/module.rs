//! Module system for mp execution modules
//!
//! This module provides the base traits and structures for creating execution modules
//! that can be run on the mp blockchain.

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::Error;

/// The execution context for a transaction
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// Execution metadata
    pub metadata: HashMap<String, String>,

    /// Transaction data
    pub transaction: HashMap<String, serde_json::Value>,

    /// Current state view
    pub state: HashMap<String, Vec<u8>>,
}

impl ExecutionContext {
    /// Create a new empty execution context
    pub fn new() -> Self {
        Self {
            metadata: HashMap::new(),
            transaction: HashMap::new(),
            state: HashMap::new(),
        }
    }

    /// Add metadata to the context
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    /// Add transaction data to the context
    pub fn with_transaction_data(mut self, key: &str, value: serde_json::Value) -> Self {
        self.transaction.insert(key.to_string(), value);
        self
    }

    /// Add state to the context
    pub fn with_state(mut self, key: &str, value: Vec<u8>) -> Self {
        self.state.insert(key.to_string(), value);
        self
    }
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Base trait for execution modules
pub trait ExecutionModule: Send + Sync {
    /// Create a new instance of this module
    fn new() -> Self
    where
        Self: Sized;

    /// Initialize the module with configuration
    fn initialize(&mut self, _config: &HashMap<String, String>) -> Result<(), Error> {
        Ok(())
    }
}

/// Trait for transaction handlers
pub trait Transaction: Send + Sync {
    /// Execute the transaction
    fn execute(&self, module: &mut dyn ExecutionModule, args: &[u8]) -> Result<Vec<u8>, Error>;
}

/// State change returned by a transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateChange {
    /// Key that was changed
    pub key: String,

    /// New value, or None if the key was deleted
    pub value: Option<Vec<u8>>,
}

/// Result of a transaction execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionResult {
    /// State changes produced by the transaction
    pub state_changes: Vec<StateChange>,

    /// Return value of the transaction
    pub return_value: Vec<u8>,

    /// Execution logs
    pub logs: Vec<String>,

    /// Error message, if any
    pub error: Option<String>,
}

/// Trait for module storage
#[async_trait]
pub trait ModuleStorage: Send + Sync {
    /// Get a value from storage
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Error>;

    /// Set a value in storage
    async fn set(&self, key: &str, value: &[u8]) -> Result<(), Error>;

    /// Delete a value from storage
    async fn delete(&self, key: &str) -> Result<(), Error>;

    /// Get multiple values from storage
    async fn get_batch(&self, keys: &[String]) -> Result<HashMap<String, Vec<u8>>, Error>;

    /// Set multiple values in storage
    async fn set_batch(&self, values: &HashMap<String, Vec<u8>>) -> Result<(), Error>;
}

#[cfg(feature = "local-storage")]
pub mod local_storage {
    use super::*;
    use diesel::prelude::*;
    use diesel::r2d2::{ConnectionManager, Pool};
    use diesel::sql_types::{Binary, Text};
    use diesel::sqlite::SqliteConnection;
    use diesel::QueryableByName;
    use std::path::Path;

    // Define a queryable struct to help with the Diesel type system
    #[derive(QueryableByName, Debug)]
    struct KeyValueRow {
        #[diesel(sql_type = Text)]
        key: String,
        #[diesel(sql_type = Binary)]
        value: Vec<u8>,
    }

    /// SQLite-based module storage
    pub struct SqliteModuleStorage {
        /// Connection pool
        pool: Pool<ConnectionManager<SqliteConnection>>,
    }

    impl SqliteModuleStorage {
        /// Create a new SQLite module storage
        pub fn new(db_path: &Path) -> Result<Self, Error> {
            // Create parent directory if it doesn't exist
            if let Some(parent) = db_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            // Create connection manager
            let manager = ConnectionManager::<SqliteConnection>::new(
                db_path.to_str().unwrap_or("./module.db"),
            );

            // Create connection pool
            let pool = Pool::builder()
                .build(manager)
                .map_err(|e| Error::Storage(format!("Failed to create connection pool: {}", e)))?;

            // Initialize database
            let mut conn = pool
                .get()
                .map_err(|e| Error::Storage(format!("Failed to get connection: {}", e)))?;

            diesel::sql_query(
                "CREATE TABLE IF NOT EXISTS module_storage (
                    key TEXT PRIMARY KEY,
                    value BLOB NOT NULL
                )",
            )
            .execute(&mut conn)
            .map_err(|e| Error::Storage(format!("Failed to create table: {}", e)))?;

            Ok(Self { pool })
        }
    }

    #[async_trait]
    impl ModuleStorage for SqliteModuleStorage {
        async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Error> {
            // Get a connection from the pool
            let mut conn = self
                .pool
                .get()
                .map_err(|e| Error::Storage(format!("Failed to get connection: {}", e)))?;

            // Create a struct for the query result
            #[derive(QueryableByName, Debug)]
            struct ValueResult {
                #[diesel(sql_type = Binary)]
                value: Vec<u8>,
            }

            // Query for the value
            let result = diesel::sql_query("SELECT value FROM module_storage WHERE key = ?")
                .bind::<Text, _>(key)
                .get_result::<ValueResult>(&mut conn)
                .optional()
                .map_err(|e| Error::Storage(format!("Failed to query storage: {}", e)))?;

            Ok(result.map(|r| r.value))
        }

        async fn set(&self, key: &str, value: &[u8]) -> Result<(), Error> {
            // Get a connection from the pool
            let mut conn = self
                .pool
                .get()
                .map_err(|e| Error::Storage(format!("Failed to get connection: {}", e)))?;

            // Insert or replace the value
            diesel::sql_query("INSERT OR REPLACE INTO module_storage (key, value) VALUES (?, ?)")
                .bind::<diesel::sql_types::Text, _>(key)
                .bind::<diesel::sql_types::Binary, _>(value)
                .execute(&mut conn)
                .map_err(|e| Error::Storage(format!("Failed to insert into storage: {}", e)))?;

            Ok(())
        }

        async fn delete(&self, key: &str) -> Result<(), Error> {
            // Get a connection from the pool
            let mut conn = self
                .pool
                .get()
                .map_err(|e| Error::Storage(format!("Failed to get connection: {}", e)))?;

            // Delete the value
            diesel::sql_query("DELETE FROM module_storage WHERE key = ?")
                .bind::<diesel::sql_types::Text, _>(key)
                .execute(&mut conn)
                .map_err(|e| Error::Storage(format!("Failed to delete from storage: {}", e)))?;

            Ok(())
        }

        async fn get_batch(&self, keys: &[String]) -> Result<HashMap<String, Vec<u8>>, Error> {
            if keys.is_empty() {
                return Ok(HashMap::new());
            }

            // Get a connection from the pool
            let mut conn = self
                .pool
                .get()
                .map_err(|e| Error::Storage(format!("Failed to get connection: {}", e)))?;

            // Build placeholders for the IN clause
            let placeholders = (0..keys.len()).map(|_| "?").collect::<Vec<_>>().join(",");

            // Query for values
            let _query = format!(
                "SELECT key, value FROM module_storage WHERE key IN ({})",
                placeholders
            );

            // We need to create a new query object for each iteration
            let mut results = Vec::new();

            for chunk in keys.chunks(5) {
                // Process in smaller batches to avoid too many parameters
                let placeholders = (0..chunk.len()).map(|_| "?").collect::<Vec<_>>().join(",");
                let _query_str = format!(
                    "SELECT key, value FROM module_storage WHERE key IN ({})",
                    placeholders
                );

                // Given the complexity with Diesel's type system for parameterized queries,
                // we'll use a simpler approach: individual queries for each key
                // This is less efficient but more robust until we solve the type system issues
                let mut chunk_results = Vec::new();

                // Process each key individually
                for key in chunk {
                    // Simple query for a single key
                    let query_str = "SELECT key, value FROM module_storage WHERE key = ?";

                    // Execute simple query
                    let results = diesel::sql_query(query_str)
                        .bind::<Text, _>(key)
                        .load::<KeyValueRow>(&mut conn)
                        .map_err(|e| {
                            Error::Storage(format!(
                                "Failed to query storage for key {}: {}",
                                key, e
                            ))
                        })?;

                    // Add results to our collection
                    chunk_results.extend(results);
                }

                results.extend(chunk_results);
            }

            // Results are now populated from chunks

            // Convert results to a HashMap
            let mut map = HashMap::new();
            for row in results {
                map.insert(row.key, row.value);
            }

            Ok(map)
        }

        async fn set_batch(&self, values: &HashMap<String, Vec<u8>>) -> Result<(), Error> {
            if values.is_empty() {
                return Ok(());
            }

            // Get a connection from the pool
            let mut conn = self
                .pool
                .get()
                .map_err(|e| Error::Storage(format!("Failed to get connection: {}", e)))?;

            // Execute each update individually since the transaction API is giving us type issues
            for (key, value) in values {
                diesel::sql_query(
                    "INSERT OR REPLACE INTO module_storage (key, value) VALUES (?, ?)",
                )
                .bind::<diesel::sql_types::Text, _>(key)
                .bind::<diesel::sql_types::Binary, _>(value.as_slice())
                .execute(&mut conn)
                .map_err(|e| Error::Storage(format!("SQL error during update: {}", e)))?;
            }

            Ok(())
        }
    }
}
