use crate::call_tree::NodeState;
pub use ethereum::TransactionV2 as Transaction;
use ethereum_types::{H160, H256};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoM {
    pub root_id: H256,
    pub challenge_id: H256,
    pub tx: Transaction,
    pub timeout: u64,
    pub caller: H160,
    pub callee: Option<H160>,
    pub call_depth: u64,
    pub state: NodeState,
}

impl PoM {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }

    pub fn from_json(json_string: &str) -> PoM {
        serde_json::from_str(json_string).unwrap()
    }
}
