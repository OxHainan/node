use chrono::Utc;
use ethereum_types::H128;
use http::{HeaderMap, HeaderValue, Method};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use sha1::Digest;
use uuid::Uuid;

use crate::types::{Transaction, TransactionType};

/// Create a new transaction with the given type and payload
pub fn create_transaction(
    tx_type: TransactionType,
    payload: Vec<u8>,
    sender: Option<String>,
    method: Method,
    header: HeaderMap<HeaderValue>,
) -> Transaction {
    Transaction {
        id: Uuid::new_v4(),
        tx_type,
        payload,
        header,
        method,
        timestamp: Utc::now(),
        sender,
        log_index: 0, // Will be set by the consensus layer
    }
}

/// Calculate a simple hash of the given data
pub fn calculate_hash(data: &[u8]) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

pub fn uuid_to_h128(uuid: &Uuid) -> H128 {
    H128::from(uuid.as_bytes())
}

pub fn h128_to_uuid(h128: &H128) -> Uuid {
    Uuid::from_bytes(h128.0)
}

pub fn string_to_uuid(target: Option<String>) -> Uuid {
    if let Some(target) = target {
        let escaped_str = utf8_percent_encode(&target, NON_ALPHANUMERIC).to_string();
        let buffer = escaped_str.into_bytes();

        let hash = sha1::Sha1::digest(&buffer);
        let mut uuid_bytes = [0u8; 16];
        uuid_bytes.copy_from_slice(&hash[..16]);
        uuid_bytes[6] = (uuid_bytes[6] & 0x0F) | 0x40; // 版本号 4
        uuid_bytes[8] = (uuid_bytes[8] & 0x3F) | 0x80; // 变体
        Uuid::from_bytes(uuid_bytes)
    } else {
        Uuid::new_v4()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_uuid_to_h128() {
        let uuid = Uuid::new_v4();
        let h128 = uuid_to_h128(&uuid);
        println!("uuid: {:?}, h128: {:?}", uuid, h128);
        assert_eq!(h128, H128::from(uuid.as_bytes()));
    }

    #[test]
    fn test_h128_to_uuid() {
        let h128 = H128::from_slice(&[0; 16]);
        let uuid = h128_to_uuid(&h128);
        assert_eq!(uuid, Uuid::from_bytes(h128.0));
    }
}
