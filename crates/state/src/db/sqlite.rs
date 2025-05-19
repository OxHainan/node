use crate::diff::{StateDiff, StateOperation};
use anyhow::{Context, Result};
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager, Pool};
use diesel::sql_types::{BigInt, Bool, Text};
use diesel::sqlite::SqliteConnection;
use diesel::QueryableByName;
use mp_common::types::Transaction;
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use tracing::info;

use crate::config::StateConfig;
use crate::diff::StateDiffStorage;
use crate::StateStorage;

/// SQLite-based state storage
pub struct SqliteStateStorage {
    config: StateConfig,
    connection_pool: Pool<ConnectionManager<SqliteConnection>>,
    state_mutex: Mutex<()>,
}

/// SQLite database schema models
mod schema {
    use diesel::sql_types::{BigInt, Integer, Text};
    use diesel::QueryableByName;

    #[derive(QueryableByName, Debug)]
    pub struct IdResult {
        #[diesel(sql_type = BigInt)]
        pub id: i64,
    }

    #[derive(QueryableByName, Debug)]
    pub struct StringResult {
        #[diesel(sql_type = Text)]
        pub root_hash: String,
    }

    #[derive(QueryableByName, Debug)]
    pub struct StateEntry {
        #[diesel(sql_type = Text)]
        pub key: String,
        #[diesel(sql_type = Text)]
        pub value: String,
        #[diesel(sql_type = Integer)]
        pub updated_at: i64,
    }

    #[derive(QueryableByName, Debug)]
    pub struct StateRoot {
        #[diesel(sql_type = Integer)]
        pub id: i64,
        #[diesel(sql_type = Text)]
        pub root_hash: String,
        #[diesel(sql_type = Text)]
        pub transaction_hash: Option<String>,
        #[diesel(sql_type = Integer)]
        pub created_at: i64,
    }

    #[derive(QueryableByName, Debug)]
    pub struct StateDiffEntry {
        #[diesel(sql_type = Integer)]
        pub id: i64,
        #[diesel(sql_type = Text)]
        pub prev_root_hash: String,
        #[diesel(sql_type = Text)]
        pub new_root_hash: String,
        #[diesel(sql_type = Integer)]
        pub created_at: i64,
    }
}

impl SqliteStateStorage {
    /// Create a new SQLite-based state storage
    pub fn new(config: StateConfig) -> Result<Self> {
        // Create database directory if it doesn't exist
        if let Some(parent) = Path::new(&config.db_connection).parent() {
            fs::create_dir_all(parent)?;
        }

        // Create state root directory if it doesn't exist
        fs::create_dir_all(&config.state_root_path)?;

        // Set up connection pool
        let manager = ConnectionManager::<SqliteConnection>::new(&config.db_connection);
        let pool = r2d2::Pool::builder().max_size(5).build(manager)?;

        // Create a simple state storage
        let storage = Self {
            config,
            connection_pool: pool,
            state_mutex: Mutex::new(()),
        };

        // Initialize the database
        storage.initialize_database()?;

        Ok(storage)
    }

    /// Initialize the database
    fn initialize_database(&self) -> Result<()> {
        // Get a connection from the pool
        let mut conn = self.connection_pool.get()?;

        // Load and execute the initial schema SQL script
        let schema_sql = include_str!("migrations/01_initial_schema.sql");

        // Split the SQL statements and execute them one by one
        for statement in schema_sql.split(';').filter(|s| !s.trim().is_empty()) {
            diesel::sql_query(statement)
                .execute(&mut conn)
                .with_context(|| format!("Failed to execute SQL: {}", statement))?;
        }

        // Create initial state root if none exists
        #[derive(QueryableByName, Debug)]
        struct ExistsFlag {
            #[sql_type = "Bool"]
            exists_flag: bool,
        }

        let root_exists =
            diesel::sql_query("SELECT EXISTS(SELECT 1 FROM state_roots LIMIT 1) as exists_flag")
                .get_result::<ExistsFlag>(&mut conn)
                .map(|flag| flag.exists_flag)
                .unwrap_or(false);

        if !root_exists {
            // Create an initial empty state root
            let initial_root = "0000000000000000000000000000000000000000000000000000000000000000";
            diesel::sql_query(
                "INSERT INTO state_roots (root_hash, transaction_hash) 
                 VALUES (?, NULL)",
            )
            .bind::<Text, _>(initial_root)
            .execute(&mut conn)?;

            info!("Created initial state root: {}", initial_root);
        }

        Ok(())
    }

    /// Calculate a simple state root hash
    fn calculate_state_root(&self) -> Result<String> {
        // Get a connection from the pool
        let mut conn = self.connection_pool.get()?;

        // This is a simplified implementation
        // In a real implementation, we would calculate a Merkle root of the state

        // Define a simple struct to hold our query results
        #[derive(QueryableByName)]
        struct KeyRow {
            #[diesel(sql_type = diesel::sql_types::Text)]
            key: String,
        }

        // Get all state keys
        let results =
            diesel::sql_query("SELECT key FROM state ORDER BY key").load::<KeyRow>(&mut conn)?;

        // Extract keys from the results
        let keys: Vec<String> = results.into_iter().map(|row| row.key).collect();

        // Calculate a simple hash
        let keys_str = keys.join(",");
        let hash = mp_common::utils::calculate_hash(keys_str.as_bytes());

        Ok(hash)
    }
}

// Implement StateDiffStorage for SqliteStateStorage
impl StateDiffStorage for SqliteStateStorage {
    fn apply_diff(&self, diff: &StateDiff) -> Result<()> {
        let mut conn = match self.connection_pool.get() {
            Ok(conn) => conn,
            Err(e) => {
                tracing::error!("Failed to get connection from pool: {}", e);
                return Err(anyhow::anyhow!("Failed to get connection from pool: {}", e));
            }
        };

        // Start transaction
        conn.transaction(|tx| {
                // First, verify the previous root exists
                #[derive(QueryableByName, Debug)]
                struct ExistsFlag {
                    #[sql_type = "Bool"]
                    exists_flag: bool,
                }
                
                let prev_root_exists = diesel::sql_query(
                    "SELECT EXISTS(SELECT 1 FROM state_roots WHERE root_hash = ? LIMIT 1) as exists_flag"
                )
                .bind::<Text, _>(&diff.prev_root)
                .get_result::<ExistsFlag>(tx)
                .map(|flag| flag.exists_flag)
                .unwrap_or(false);
                
                if !prev_root_exists {
                    return Err(diesel::result::Error::NotFound.into());
                }
                
                // Insert new root if it doesn't exist
                let new_root_exists = diesel::sql_query(
                    "SELECT EXISTS(SELECT 1 FROM state_roots WHERE root_hash = ? LIMIT 1) as exists_flag"
                )
                .bind::<Text, _>(&diff.new_root)
                .get_result::<ExistsFlag>(tx)
                .map(|flag| flag.exists_flag)
                .unwrap_or(false);
                
                if !new_root_exists {
                    diesel::sql_query(
                        "INSERT INTO state_roots (root_hash, transaction_hash) 
                        VALUES (?, NULL)"
                    )
                    .bind::<Text, _>(&diff.new_root)
                    .execute(tx)?;
                }
                
                // Insert the diff record
                let diff_id = diesel::sql_query(
                    "INSERT INTO state_diffs (prev_root_hash, new_root_hash) 
                    VALUES (?, ?) RETURNING id"
                )
                .bind::<Text, _>(&diff.prev_root)
                .bind::<Text, _>(&diff.new_root)
                .get_result::<schema::IdResult>(tx)?
                .id;
                
                // Process each operation in the diff
                for op in &diff.operations {
                    match op {
                        StateOperation::Insert { key, value } => {
                            // Store the operation in state_operations
                            diesel::sql_query(
                                "INSERT INTO state_operations (diff_id, operation_type, key, value) 
                                VALUES (?, 'insert', ?, ?)"
                            )
                            .bind::<BigInt, _>(diff_id)
                            .bind::<Text, _>(key)
                            .bind::<Text, _>(value)
                            .execute(tx)?;
                            
                            // Apply the operation to state_entries
                            diesel::sql_query(
                                "INSERT OR REPLACE INTO state_entries (key, value) 
                                VALUES (?, ?)"
                            )
                            .bind::<Text, _>(key)
                            .bind::<Text, _>(value)
                            .execute(tx)?;
                        }
                        StateOperation::Delete { key } => {
                            // Store the operation in state_operations
                            diesel::sql_query(
                                "INSERT INTO state_operations (diff_id, operation_type, key) 
                                VALUES (?, 'delete', ?)"
                            )
                            .bind::<BigInt, _>(diff_id)
                            .bind::<Text, _>(key)
                            .execute(tx)?;
                            
                            // Apply the operation to state_entries
                            diesel::sql_query(
                                "DELETE FROM state_entries WHERE key = ?"
                            )
                            .bind::<Text, _>(key)
                            .execute(tx)?;
                        }
                    }
                }

                Ok(())
            })
    }

    fn create_checkpoint(&self) -> Result<StateDiff> {
        let mut conn = self.connection_pool.get()?;

        // Get the current state root
        let root_hash =
            diesel::sql_query("SELECT root_hash FROM state_roots ORDER BY id DESC LIMIT 1")
                .get_result::<schema::StringResult>(&mut conn)?
                .root_hash;

        // Create an empty diff checkpoint with the current root
        Ok(StateDiff {
            prev_root: root_hash.clone(),
            new_root: root_hash,
            operations: Vec::new(),
        })
    }
}

impl StateStorage for SqliteStateStorage {
    fn start(&self) -> Result<()> {
        info!("Starting SQLite state storage");
        self.initialize_database()?;
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        info!("Stopping SQLite state storage");
        Ok(())
    }

    fn apply_transaction(&self, transaction: Transaction) -> Result<()> {
        info!("Applying transaction: {:?}", transaction.id);

        // Get a connection from the pool
        let mut conn = self.connection_pool.get()?;

        // Use a mutex to ensure only one transaction is processed at a time
        let _lock = self.state_mutex.lock().unwrap();

        // Store the transaction
        diesel::sql_query(
            "INSERT INTO transactions (id, type, payload, timestamp, sender)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind::<diesel::sql_types::Text, _>(transaction.id.to_string())
        .bind::<diesel::sql_types::Text, _>(format!("{:?}", transaction.tx_type))
        .bind::<diesel::sql_types::Binary, _>(transaction.payload)
        .bind::<diesel::sql_types::Text, _>(transaction.timestamp.to_rfc3339())
        .bind::<diesel::sql_types::Nullable<diesel::sql_types::Text>, _>(transaction.sender)
        .execute(&mut conn)?;

        // Update the state root
        let state_root = self.calculate_state_root()?;
        let state_root_path = Path::new(&self.config.state_root_path).join("state_root");
        fs::write(state_root_path, state_root.as_bytes())?;

        Ok(())
    }

    fn get_state_root(&self) -> Result<String> {
        let state_root = self.calculate_state_root()?;
        Ok(state_root)
    }

    fn clone(&self) -> std::sync::Arc<dyn StateStorage> {
        std::sync::Arc::new(SqliteStateStorage {
            config: self.config.clone(),
            connection_pool: self.connection_pool.clone(),
            state_mutex: Mutex::new(()),
        })
    }
}
