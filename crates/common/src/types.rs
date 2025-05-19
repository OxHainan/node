use async_raft::{AppData, AppDataResponse};
use chrono::{DateTime, Utc};
use ethereum_types::H128;
use http::{HeaderMap, HeaderValue, Method};
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cmp::Ordering;
use std::fmt;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestParams {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub args: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub module: Option<String>,
}

/// Transaction types supported by the blockchain
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionType {
    /// Web2 API request
    Request(H128, String), // (请求的合约地址, 请求的路径)
    /// State change operation
    StateChange,
    /// Scheduled task execution
    ScheduledTask,
    /// Create container
    CreateContainer,
    /// Start container
    StartContainer,
    /// Stop container
    StopContainer,
    /// List containers
    ListContainers,
    /// remove container
    RemoveContainer,
}

impl TransactionType {
    pub fn is_request(&self) -> bool {
        matches!(self, TransactionType::Request(_, _))
    }
}

impl Serialize for TransactionType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            TransactionType::Request(address, method) => {
                serializer.serialize_str(&format!("/{:?}/{}", address, method))
            }
            TransactionType::StateChange => serializer.serialize_str("/cvm/state_change"),
            TransactionType::ScheduledTask => serializer.serialize_str("/cvm/scheduled_task"),
            TransactionType::CreateContainer => serializer.serialize_str("/cvm/create_container"),
            TransactionType::StopContainer => serializer.serialize_str("/cvm/stop_container"),
            TransactionType::StartContainer => serializer.serialize_str("/cvm/start_container"),
            TransactionType::ListContainers => serializer.serialize_str("/cvm/list_containers"),
            TransactionType::RemoveContainer => serializer.serialize_str("/cvm/remove_container"),
        }
    }
}

/// 自定义反序列化
impl<'de> Deserialize<'de> for TransactionType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct TransactionTypeVisitor;

        impl<'de> Visitor<'de> for TransactionTypeVisitor {
            type Value = TransactionType;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid transaction type string")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match value {
                    "/cvm/state_change" => return Ok(TransactionType::StateChange),
                    "/cvm/scheduled_task" => return Ok(TransactionType::ScheduledTask),
                    "/cvm/create_container" => return Ok(TransactionType::CreateContainer),
                    "/cvm/stop_container" => return Ok(TransactionType::StopContainer),
                    "/cvm/list_containers" => return Ok(TransactionType::ListContainers),
                    "/cvm/remove_container" => return Ok(TransactionType::RemoveContainer),
                    "/cvm/start_container" => return Ok(TransactionType::StartContainer),
                    _ => {} // 未知值默认解析为 Request
                }

                // 其他情况，尝试解析成 Request
                let value = value.strip_prefix('/').unwrap_or(value); // 去掉开头的 /
                let mut parts = value.splitn(2, '/'); // 分成两部分

                let address_str = parts.next().ok_or_else(|| E::custom("missing address"))?;
                let method = parts.next().unwrap_or("").to_string(); // 可能没有方法名，就空字符串

                // 解析0x开头的地址
                let address = if address_str.starts_with("0x") {
                    address_str
                        .parse::<H128>()
                        .map_err(|_| E::custom("invalid H128 address"))?
                } else {
                    return Err(E::custom("invalid address format"));
                };

                Ok(TransactionType::Request(address, method))
            }
        }

        deserializer.deserialize_str(TransactionTypeVisitor)
    }
}

impl TransactionType {
    pub fn parse(path: &str) -> Option<Self> {
        let path = path.strip_prefix('/').unwrap_or(path); // 去掉最前面的 /
        if path.starts_with("cvm/") {
            match path {
                "cvm/state_change" => Some(TransactionType::StateChange),
                "cvm/scheduled_task" => Some(TransactionType::ScheduledTask),
                "cvm/create_container" => Some(TransactionType::CreateContainer),
                "cvm/stop_container" => Some(TransactionType::StopContainer),
                "cvm/start_container" => Some(TransactionType::StartContainer),
                "cvm/list_containers" => Some(TransactionType::ListContainers),
                "cvm/remove_container" => Some(TransactionType::RemoveContainer),
                _ => None,
            }
        } else if path.starts_with("0x") {
            // 拿到0x后面的地址和方法
            let mut parts = path.splitn(2, '/'); // 只分成两部分
            let address_part = parts.next()?;
            let method_part = parts.next().unwrap_or(""); // 可能没有方法

            if let Ok(address) = address_part.parse::<H128>() {
                Some(TransactionType::Request(address, method_part.to_string()))
            } else {
                None
            }
        } else {
            None
        }
    }
}

/// A unified transaction format for all operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    /// Unique transaction ID
    pub id: Uuid,
    #[serde(skip)]
    pub method: Method,
    #[serde(skip)]
    pub header: HeaderMap<HeaderValue>,
    /// Transaction type
    pub tx_type: TransactionType,
    /// Transaction payload (serialized data)
    pub payload: Vec<u8>,
    /// Transaction timestamp
    pub timestamp: DateTime<Utc>,
    /// Transaction sender (if applicable)
    pub sender: Option<String>,
    /// Index in the Raft log (used by consensus)
    #[serde(default)]
    pub log_index: u64,
}

#[derive(Debug, Clone)]
pub enum TransactionStatusWithProof {
    /// Transaction is pending
    Pending,
    /// Transaction is being processed
    Processing,
    /// Transaction has been confirmed
    Confirmed(
        serde_json::Value,
        u16,
        Option<HeaderMap>,
        Option<serde_json::Value>,
    ),
    /// Transaction has failed
    Failed(
        serde_json::Value,
        u16,
        Option<HeaderMap>,
        Option<serde_json::Value>,
    ),
}

// Implement PartialEq for Transaction based on ID
impl PartialEq for Transaction {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

// Implement Eq for Transaction
impl Eq for Transaction {}

// Implement Ord for Transaction based on log_index
impl Ord for Transaction {
    fn cmp(&self, other: &Self) -> Ordering {
        self.log_index.cmp(&other.log_index)
    }
}

// Implement PartialOrd for Transaction
impl PartialOrd for Transaction {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// Implement Display for Transaction
impl fmt::Display for Transaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Transaction(id={}, type={:?}, index={})",
            self.id, self.tx_type, self.log_index
        )
    }
}

// 实现async-raft所需的AppData特性
impl AppData for Transaction {}

/// API request transaction payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiRequestPayload {
    /// HTTP method
    pub method: String,
    // /// API path
    // pub path: String,
    /// Request headers
    pub headers: Vec<(String, String)>,
    /// Request body
    pub body: Vec<u8>,
}

/// State change transaction payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateChangePayload {
    /// Contract ID
    pub contract_id: String,
    /// Database operation (SQL or other format)
    pub operation: String,
    /// Previous state hash
    pub previous_state_hash: String,
}

/// Scheduled task transaction payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTaskPayload {
    /// Contract ID
    pub contract_id: String,
    /// Task name
    pub task_name: String,
    /// Next execution time
    pub next_execution: DateTime<Utc>,
    /// Execution interval in seconds (0 for one-time tasks)
    pub interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TransactionStatus {
    Pending,
    Processing,
    Success,
    Error,
}

/// Transaction response for API and Raft consensus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionResponse {
    /// Transaction ID
    pub tx_id: Uuid,
    /// Transaction status (pending, processing, success, error)
    pub status: TransactionStatus,
    /// Transaction result (if available)
    pub result: Option<serde_json::Value>,
}

impl TransactionResponse {
    /// Create a successful response
    pub fn success(tx_id: Uuid) -> Self {
        Self {
            tx_id,
            status: TransactionStatus::Success,
            result: None,
        }
    }

    /// Create a successful response with result
    pub fn success_with_result(tx_id: Uuid, result: serde_json::Value) -> Self {
        Self {
            tx_id,
            status: TransactionStatus::Success,
            result: Some(result),
        }
    }

    /// Create an error response
    pub fn error(message: String) -> Self {
        Self {
            tx_id: Uuid::new_v4(),
            status: TransactionStatus::Error,
            result: Some(serde_json::Value::String(message)),
        }
    }
}

// 实现async-raft所需的AppDataResponse特性
impl AppDataResponse for TransactionResponse {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_cvm() {
        let json = r#""/cvm/state_change""#;
        let tx: TransactionType = serde_json::from_str(json).unwrap();
        assert_eq!(tx, TransactionType::StateChange);
    }

    #[test]
    fn test_deserialize_request_with_method() {
        let json = r#""/0x1234567890abcdef1234567890abcdef/api/chat/completes""#;
        let tx: TransactionType = serde_json::from_str(json).unwrap();
        if let TransactionType::Request(address, method) = tx {
            assert_eq!(
                address,
                "0x1234567890abcdef1234567890abcdef"
                    .parse::<H128>()
                    .unwrap()
            );
            assert_eq!(method, "api/chat/completes");
        } else {
            panic!("Expected Request variant");
        }
    }

    #[test]
    fn test_deserialize_request_without_method() {
        let json = r#""/0x1234567890abcdef1234567890abcdef""#;
        let tx: TransactionType = serde_json::from_str(json).unwrap();
        if let TransactionType::Request(address, method) = tx {
            assert_eq!(
                address,
                "0x1234567890abcdef1234567890abcdef"
                    .parse::<H128>()
                    .unwrap()
            );
            assert_eq!(method, ""); // method为空
        } else {
            panic!("Expected Request variant");
        }
    }

    #[test]
    fn test_deserialize_invalid_address() {
        let json = r#""/notanaddress/method""#;
        let result: Result<TransactionType, _> = serde_json::from_str(json);
        assert!(result.is_err()); // 应该出错
    }

    #[test]
    fn test_serialize_request() {
        let tx = TransactionType::Request(
            "0x1234567890abcdef1234567890abcdef"
                .parse::<H128>()
                .unwrap(),
            "api/chat/completes".to_string(),
        );
        let serialized = serde_json::to_string(&tx).unwrap();
        assert_eq!(
            serialized,
            r#""/0x1234567890abcdef1234567890abcdef/api/chat/completes""#
        );

        let tx = TransactionType::parse("/0x1234567890abcdef1234567890abcdef/api/chat/completes")
            .unwrap();
        assert_eq!(
            tx,
            TransactionType::Request(
                "0x1234567890abcdef1234567890abcdef"
                    .parse::<H128>()
                    .unwrap(),
                "api/chat/completes".to_string()
            )
        );
    }

    #[test]
    fn test_serialize_cvm() {
        let tx = TransactionType::ScheduledTask;
        let serialized = serde_json::to_string(&tx).unwrap();
        assert_eq!(serialized, r#""/cvm/scheduled_task""#);

        let tx = TransactionType::parse("/cvm/scheduled_task").unwrap();
        assert_eq!(tx, TransactionType::ScheduledTask);
    }
}
