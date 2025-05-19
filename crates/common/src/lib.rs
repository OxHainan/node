pub mod error;
pub mod types;
pub mod utils;

pub use error::Error;
pub use ethereum_types::H128;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Clone, Serialize, Default)]
pub struct TransactionResponse {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub entity_diffs: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub state_diffs: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub status_code: Option<u32>,
    #[serde(flatten)]
    pub output: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_transaction_response() {
        let json_data = json!({
            "entity_diffs": [
                {
                    "action": "update",
                    "data": {
                    "created_at": "2025-04-23T09:00:05.481942466Z",
                    "email": "perf1@example.com",
                    "id": "perf_user1",
                    "name": "Performance Test User 1",
                    "updated_at": "2025-04-23T09:00:05.481943108Z"
                    },
                    "entity_type": "users",
                    "id": "perf_user1"
                }
            ],
            "state_diffs": [
                {
                    "key": "users/perf_user1/created_at",
                    "new_value": "2025-04-23T09:00:05.481942466Z",
                    "old_value": null
                },
                {
                    "key": "users/perf_user1/email",
                    "new_value": "perf1@example.com",
                    "old_value": null
                },
                {
                    "key": "users/perf_user1/id",
                    "new_value": "perf_user1",
                    "old_value": null
                },
                {
                    "key": "users/perf_user1/name",
                    "new_value": "Performance Test User 1",
                    "old_value": null
                },
                {
                    "key": "users/perf_user1/updated_at",
                "new_value": "2025-04-23T09:00:05.481943108Z",
                "old_value": null
                }
            ],
            "status_code": 201,
            "transaction_id": "bacf5e6a-b052-4792-815c-d7707c896895",
            "user": {
            "created_at": "2025-04-23T09:00:05.481942466Z",
            "email": "perf1@example.com",
            "id": "perf_user1",
            "name": "Performance Test User 1",
            "updated_at": "2025-04-23T09:00:05.481943108Z"
            }
        });

        let transaction_response: TransactionResponse = serde_json::from_value(json_data).unwrap();
        println!("TransactionResponse: {:#?}", transaction_response.output);
        println!(
            "TransactionResponse: {}",
            serde_json::to_string_pretty(&transaction_response).unwrap()
        );
    }

    #[test]
    fn test_openai_response() {
        let json_data = json!({
            "result":  {
                "error":  {
                    "code": "invalid_api_key",
                    "message": "Incorrect API key provided: sk-proj-********************************************************************************************************************************************************DqIA. You can find your API key at https://platform.openai.com/account/api-keys.",
                    "param": null,
                    "type": "invalid_request_error"
                }
            },
            "status_code": 401
        });

        let transaction_response: TransactionResponse = serde_json::from_value(json_data).unwrap();
        println!("TransactionResponse: {:#?}", transaction_response);
    }
}
