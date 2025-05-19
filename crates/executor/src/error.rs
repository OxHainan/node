use thiserror::Error;

/// Error types for the execution engine
#[derive(Debug, Error)]
pub enum ExecutionError {
    /// Module not found
    #[error("Module not found: {0}")]
    ModuleNotFound(String),

    /// Handler not found in module
    #[error("Handler not found: {0}")]
    HandlerNotFound(String),

    /// Invalid input format
    #[error("Invalid input format: {0}")]
    InvalidInput(String),

    /// Execution timed out
    #[error("Execution timed out after {0}ms")]
    ExecutionTimeout(u64),

    /// Resource limit exceeded
    #[error("Resource limit exceeded: {0}")]
    ResourceLimitExceeded(String),

    /// Internal execution error
    #[error("Execution error: {0}")]
    ExecutionError(String),

    /// State access error
    #[error("State access error: {0}")]
    StateError(String),

    /// Communication error
    #[error("Communication error: {0}")]
    CommunicationError(String),

    /// Internal error
    #[error("Internal error: {0}")]
    InternalError(String),
}
