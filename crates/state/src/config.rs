use serde::{Deserialize, Serialize};

/// State storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateConfig {
    /// Database type
    pub db_type: String,

    /// Database connection string
    pub db_connection: String,

    /// State root storage path
    pub state_root_path: String,
}
