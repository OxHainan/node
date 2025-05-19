use ethereum::{LegacyTransaction, TransactionAction, TransactionSignature};
use ethereum_types::{H160, H256, U256};
use mp_pom::{
    call_tree::{self, list_all_call_trees, CallTreeState, NodeState},
    model::{PoM, Transaction},
};
use std::str::FromStr;

/// Create a test PoM object
fn create_test_pom(challenge_id: H256, root_id: H256, call_depth: u64, state: NodeState) -> PoM {
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

    // Create PoM instance
    PoM {
        root_id,
        challenge_id,
        tx,
        timeout: 300,
        caller: H160::from_slice(&[3u8; 20]),
        callee: Some(H160::from_slice(&[4u8; 20])),
        call_depth,
        state,
    }
}

#[test]
fn test_call_tree_state_transitions() {
    // Create a root ID to identify the entire challenge tree
    let root_id = H256::from_slice(&[1u8; 32]);

    // Create multiple challenges with different depths
    let challenge_id_1 = H256::from_slice(&[2u8; 32]);
    let challenge_id_2 = H256::from_slice(&[3u8; 32]);
    let challenge_id_3 = H256::from_slice(&[4u8; 32]);

    // Create PoM objects with different depths to simulate challenges
    let pom_1 = create_test_pom(challenge_id_1, root_id, 0, NodeState::Challenging);
    let pom_2 = create_test_pom(challenge_id_2, root_id, 1, NodeState::Challenging);
    let pom_3 = create_test_pom(challenge_id_3, root_id, 2, NodeState::Challenging);

    // 1. Test challenge processing
    // Process the first challenge
    call_tree::handle_challenge(pom_1.clone(), 100);

    // Verify call_tree state
    let call_trees = list_all_call_trees();
    let call_tree = call_trees.get(&root_id).expect("Call tree should exist");
    assert_eq!(call_tree.state, CallTreeState::Challenging);
    assert_eq!(call_tree.active_depth, Some(0));
    assert_eq!(call_tree.deepest_challenge, challenge_id_1);

    // Process the second challenge (deeper depth)
    call_tree::handle_challenge(pom_2.clone(), 110);

    // Verify call_tree state update
    let call_trees = list_all_call_trees();
    let call_tree = call_trees.get(&root_id).expect("Call tree should exist");
    assert_eq!(call_tree.state, CallTreeState::Challenging);
    assert_eq!(call_tree.active_depth, Some(1));
    assert_eq!(call_tree.deepest_challenge, challenge_id_2);
    assert_eq!(call_tree.sec_deepest_challenge, challenge_id_1);

    // Process the third challenge (deepest depth)
    call_tree::handle_challenge(pom_3.clone(), 120);

    // Verify call_tree state update again
    let call_trees = list_all_call_trees();
    let call_tree = call_trees.get(&root_id).expect("Call tree should exist");
    assert_eq!(call_tree.state, CallTreeState::Challenging);
    assert_eq!(call_tree.active_depth, Some(2));
    assert_eq!(call_tree.deepest_challenge, challenge_id_3);
    assert_eq!(call_tree.sec_deepest_challenge, challenge_id_2);

    // 2. Test response handling
    // Create a response PoM (responding to the deepest challenge)
    let response_pom_3 = create_test_pom(challenge_id_3, root_id, 2, NodeState::Responsed);
    call_tree::handle_response(response_pom_3, 130);

    // Verify call_tree state update (deepest challenge is responded, second deepest becomes deepest)
    let call_trees = list_all_call_trees();
    let call_tree = call_trees.get(&root_id).expect("Call tree should exist");
    assert_eq!(call_tree.state, CallTreeState::Challenging);
    assert_eq!(call_tree.active_depth, Some(1));
    assert_eq!(call_tree.deepest_challenge, challenge_id_2);

    // Respond to the second deepest challenge
    let response_pom_2 = create_test_pom(challenge_id_2, root_id, 1, NodeState::Responsed);
    call_tree::handle_response(response_pom_2, 140);

    // Verify call_tree state update
    let call_trees = list_all_call_trees();
    let call_tree = call_trees.get(&root_id).expect("Call tree should exist");
    assert_eq!(call_tree.state, CallTreeState::Challenging);
    assert_eq!(call_tree.active_depth, Some(0));
    assert_eq!(call_tree.deepest_challenge, challenge_id_1);

    // 3. Test timeout handling and punishment mechanism
    // Create a timeout block height
    let timeout_block = 400; // Exceeds pom_1's timeout

    // Check timeout and punish
    call_tree::check_timeout_and_punish(pom_1.clone(), timeout_block);

    // Verify call_tree state update (all challenges are responded or punished, state becomes Punished)
    let call_trees = list_all_call_trees();
    let call_tree = call_trees.get(&root_id).expect("Call tree should exist");
    assert_eq!(call_tree.state, CallTreeState::Punished);
    assert_eq!(call_tree.active_depth, None);

    // 4. Test edge case: duplicate challenge_id
    // Create a PoM with an existing challenge_id
    let duplicate_pom = create_test_pom(challenge_id_1, root_id, 0, NodeState::Challenging);
    call_tree::handle_challenge(duplicate_pom, 150);

    // Verify call_tree state remains unchanged (duplicate challenge_id is ignored)
    let call_trees = list_all_call_trees();
    let call_tree = call_trees.get(&root_id).expect("Call tree should exist");
    assert_eq!(call_tree.state, CallTreeState::Punished);

    // 5. Test new challenge tree
    // Create a new root ID
    let new_root_id = H256::from_slice(&[5u8; 32]);
    let new_challenge_id = H256::from_slice(&[6u8; 32]);

    // Create a new PoM object
    let new_pom = create_test_pom(new_challenge_id, new_root_id, 0, NodeState::Challenging);

    // Process the new challenge
    call_tree::handle_challenge(new_pom.clone(), 160);

    // Verify a new call_tree is created
    let call_trees = list_all_call_trees();
    assert!(call_trees.contains_key(&new_root_id));

    let new_call_tree = call_trees
        .get(&new_root_id)
        .expect("New call tree should exist");
    assert_eq!(new_call_tree.state, CallTreeState::Challenging);
    assert_eq!(new_call_tree.active_depth, Some(0));
    assert_eq!(new_call_tree.deepest_challenge, new_challenge_id);
}

#[test]
fn test_deep_challenge_response_sequence() {
    // Create a root ID
    let root_id = H256::from_slice(&[10u8; 32]);

    // Create a series of challenges with increasing depth
    let mut challenge_ids = Vec::new();
    let mut poms = Vec::new();

    // Create 10 challenges with different depths
    for i in 0..10 {
        let challenge_id = H256::from_slice(&[i as u8 + 20; 32]);
        challenge_ids.push(challenge_id);

        let pom = create_test_pom(challenge_id, root_id, i, NodeState::Challenging);
        poms.push(pom);
    }

    // Process all challenges in sequence
    for (i, pom) in poms.iter().enumerate() {
        call_tree::handle_challenge(pom.clone(), 100 + i as u64);

        // Verify state after each processing
        let call_trees = list_all_call_trees();
        let call_tree = call_trees.get(&root_id).expect("Call tree should exist");
        assert_eq!(call_tree.state, CallTreeState::Challenging);
        assert_eq!(call_tree.active_depth, Some(i as u64));
        assert_eq!(call_tree.deepest_challenge, challenge_ids[i]);
    }

    // Starting from the deepest challenge, respond to each one
    for i in (0..10).rev() {
        // Create response
        let response_pom =
            create_test_pom(challenge_ids[i], root_id, i as u64, NodeState::Responsed);
        call_tree::handle_response(response_pom, 200 + i as u64);

        // Verify state
        let call_trees = list_all_call_trees();
        let call_tree = call_trees.get(&root_id).expect("Call tree should exist");

        if i > 0 {
            // If there are still unresponded challenges
            assert_eq!(call_tree.state, CallTreeState::Challenging);
            assert_eq!(call_tree.active_depth, Some((i - 1) as u64));
            assert_eq!(call_tree.deepest_challenge, challenge_ids[i - 1]);
        } else {
            // All challenges have been responded
            assert_eq!(call_tree.state, CallTreeState::Responsed);
            assert_eq!(call_tree.active_depth, None);
        }
    }
}

#[test]
fn test_timeout_punishment_mechanism() {
    // Create a root ID
    let root_id = H256::from_slice(&[11u8; 32]);

    // Create 3 challenges with different depths
    let challenge_id_1 = H256::from_slice(&[12u8; 32]);
    let challenge_id_2 = H256::from_slice(&[13u8; 32]);
    let challenge_id_3 = H256::from_slice(&[14u8; 32]);

    // Create PoM objects with different timeout values
    let mut pom_1 = create_test_pom(challenge_id_1, root_id, 0, NodeState::Challenging);
    let mut pom_2 = create_test_pom(challenge_id_2, root_id, 1, NodeState::Challenging);
    let mut pom_3 = create_test_pom(challenge_id_3, root_id, 2, NodeState::Challenging);

    // Set different timeout values
    pom_1.timeout = 150;
    pom_2.timeout = 200;
    pom_3.timeout = 250;

    // Process all challenges
    call_tree::handle_challenge(pom_1.clone(), 100);
    call_tree::handle_challenge(pom_2.clone(), 110);
    call_tree::handle_challenge(pom_3.clone(), 120);

    // Verify initial state
    let call_trees = list_all_call_trees();
    let call_tree = call_trees.get(&root_id).expect("Call tree should exist");
    assert_eq!(call_tree.state, CallTreeState::Challenging);
    assert_eq!(call_tree.active_depth, Some(2));
    assert_eq!(call_tree.deepest_challenge, challenge_id_3);

    // Test check before timeout (should not punish)
    call_tree::check_timeout_and_punish(pom_3.clone(), 240);

    // Verify state remains unchanged
    let call_trees = list_all_call_trees();
    let call_tree = call_trees.get(&root_id).expect("Call tree should exist");
    assert_eq!(call_tree.state, CallTreeState::Challenging);
    assert_eq!(call_tree.active_depth, Some(2));

    // Test check after timeout (should punish the deepest challenge)
    call_tree::check_timeout_and_punish(pom_3.clone(), 260);

    // Verify the deepest challenge is punished, and state is updated
    let call_trees = list_all_call_trees();
    let call_tree = call_trees.get(&root_id).expect("Call tree should exist");
    assert_eq!(call_tree.state, CallTreeState::Punished);
    assert_eq!(call_tree.active_depth, None);
    assert_eq!(call_tree.deepest_challenge, H256::zero());

    // Test timeout for the second deepest challenge
    call_tree::check_timeout_and_punish(pom_2.clone(), 210);

    // Verify the second deepest challenge is punished, shallowest becomes deepest
    let call_trees = list_all_call_trees();
    let call_tree = call_trees.get(&root_id).expect("Call tree should exist");
    assert_eq!(call_tree.state, CallTreeState::Punished);
    assert_eq!(call_tree.active_depth, None);
    assert_eq!(call_tree.deepest_challenge, H256::zero());

    // Test timeout for the last challenge
    call_tree::check_timeout_and_punish(pom_1.clone(), 160);

    // Verify all challenges are punished, state becomes Punished
    let call_trees = list_all_call_trees();
    let call_tree = call_trees.get(&root_id).expect("Call tree should exist");
    assert_eq!(call_tree.state, CallTreeState::Punished);
    assert_eq!(call_tree.active_depth, None);
}
