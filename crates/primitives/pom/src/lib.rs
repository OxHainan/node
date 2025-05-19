#[allow(dead_code)]
pub mod call_tree;
pub mod config;
#[allow(dead_code)]
mod event_handler;
#[allow(dead_code, unused_variables)]
pub mod l1_helper;
pub mod model;

#[cfg(test)]
mod tests {
    use super::*;
    use ethereum::{LegacyTransaction, TransactionAction, TransactionSignature};
    use ethereum_types::{H160, H256, U256};
    use model::{PoM, Transaction};

    /// Test PoM creation and serialization
    #[test]
    fn test_pom_creation_and_serialization() {
        // Create a test transaction
        let tx = Transaction::Legacy(LegacyTransaction {
            nonce: U256::from(1),
            gas_price: U256::from(1000000000),
            gas_limit: U256::from(21000),
            action: TransactionAction::Call(H160::from_slice(&[1u8; 20])),
            value: U256::zero(),
            input: vec![].into(),
            signature: TransactionSignature::new(
                27,                           // v value
                H256::from_slice(&[1u8; 32]), // r value
                H256::from_slice(&[2u8; 32]), // s value
            )
            .unwrap(),
        });

        // Create a PoM instance
        let pom = PoM {
            root_id: H256::from_slice(&[1u8; 32]),
            challenge_id: H256::from_slice(&[2u8; 32]),
            tx,
            timeout: 300, // 5 minutes
            caller: H160::from_slice(&[3u8; 20]),
            callee: Some(H160::from_slice(&[4u8; 20])),
            call_depth: 0,
            state: call_tree::NodeState::Challenging,
        };

        // Test JSON serialization
        let json_string = pom.to_json();
        assert!(!json_string.is_empty());

        // Test JSON deserialization
        let deserialized_pom = PoM::from_json(&json_string);
        assert_eq!(deserialized_pom.root_id, pom.root_id);
        assert_eq!(deserialized_pom.challenge_id, pom.challenge_id);
        assert_eq!(deserialized_pom.timeout, pom.timeout);
        assert_eq!(deserialized_pom.caller, pom.caller);
        assert_eq!(deserialized_pom.callee, pom.callee);
        assert_eq!(deserialized_pom.state, pom.state);
    }

    /// Test PoM state transitions
    #[test]
    fn test_pom_state_transitions() {
        // Create a test transaction
        let tx = Transaction::Legacy(LegacyTransaction {
            nonce: U256::from(1),
            gas_price: U256::from(1000000000),
            gas_limit: U256::from(21000),
            action: TransactionAction::Call(H160::from_slice(&[1u8; 20])),
            value: U256::zero(),
            input: vec![].into(),
            signature: TransactionSignature::new(
                27,                           // v value
                H256::from_slice(&[1u8; 32]), // r value
                H256::from_slice(&[2u8; 32]), // s value
            )
            .unwrap(),
        });

        let mut pom = PoM {
            root_id: H256::from_slice(&[1u8; 32]),
            challenge_id: H256::from_slice(&[2u8; 32]),
            tx,
            timeout: 300,
            caller: H160::from_slice(&[3u8; 20]),
            callee: Some(H160::from_slice(&[4u8; 20])),
            call_depth: 0,
            state: call_tree::NodeState::Challenging,
        };

        // Test state transitions
        assert_eq!(pom.state, call_tree::NodeState::Challenging);
        pom.state = call_tree::NodeState::Frozen;
        assert_eq!(pom.state, call_tree::NodeState::Frozen);
        pom.state = call_tree::NodeState::Responsed;
        assert_eq!(pom.state, call_tree::NodeState::Responsed);
        pom.state = call_tree::NodeState::Punished;
        assert_eq!(pom.state, call_tree::NodeState::Punished);
    }
}
