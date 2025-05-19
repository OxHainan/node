use std::fmt;

// 添加 axum 相关导入
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

// 框架错误类型
#[derive(Debug)]
pub enum Error {
    DatabaseError(String),
    HttpError(String),
    SerializationError(String),
    DeserializationError(String),
    ValidationError(String),
    NotFound(String),
    BadRequest(String),
    Unauthorized(String),
    Forbidden(String),
    InternalError(String),
    BlockchainError(String),
    IoError(String),
    NotificationError(String),
    NotImplemented(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            Error::HttpError(msg) => write!(f, "HTTP error: {}", msg),
            Error::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            Error::DeserializationError(msg) => write!(f, "Deserialization error: {}", msg),
            Error::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            Error::NotFound(msg) => write!(f, "Not found: {}", msg),
            Error::BadRequest(msg) => write!(f, "Bad request: {}", msg),
            Error::Unauthorized(msg) => write!(f, "Unauthorized: {}", msg),
            Error::Forbidden(msg) => write!(f, "Forbidden: {}", msg),
            Error::InternalError(msg) => write!(f, "Internal error: {}", msg),
            Error::BlockchainError(msg) => write!(f, "Blockchain error: {}", msg),
            Error::IoError(msg) => write!(f, "IO error: {}", msg),
            Error::NotificationError(msg) => write!(f, "Notification error: {}", msg),
            Error::NotImplemented(msg) => write!(f, "Not implemented: {}", msg),
        }
    }
}

impl std::error::Error for Error {}

// 实现 axum 的 IntoResponse trait
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            Error::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            Error::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            Error::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg),
            Error::Forbidden(msg) => (StatusCode::FORBIDDEN, msg),
            Error::ValidationError(msg) => (StatusCode::BAD_REQUEST, msg),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };

        let body = serde_json::json!({
            "status_code": status.as_u16(),
            "error": error_message,
        });

        (status, axum::Json(body)).into_response()
    }
}

// 从字符串转换为错误
impl From<String> for Error {
    fn from(s: String) -> Self {
        Error::InternalError(s)
    }
}

// 从&str转换为错误
impl From<&str> for Error {
    fn from(s: &str) -> Self {
        Error::InternalError(s.to_string())
    }
}

// 从serde_json::Error转换为错误
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::SerializationError(e.to_string())
    }
}

// 框架结果类型
pub type Result<T> = std::result::Result<T, Error>;
