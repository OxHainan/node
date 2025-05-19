use crate::model::PoM;
use ethereum_types::{H160, H256};
use lazy_static::lazy_static;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use tokio::runtime::Runtime;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeState {
    Default,
    Challenging,
    Responsed,
    Punished,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum CallTreeState {
    Challenging,
    Responsed,
    Timeout,
    Punished,
    Default, // Add Default variant
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Node {
    challenge_id: H256,
    root_id: H256,
    timeout: u64,
    caller: H160,
    callee: Option<H160>,
    call_depth: u64,
    block_number: u64,
    state: NodeState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallTree {
    pub nodes: Vec<Node>,
    pub state: CallTreeState,
    pub active_depth: Option<u64>,
    pub deepest_challenge: H256,
    pub sec_deepest_challenge: H256,
    pub block_number: u64,
}

impl CallTree {
    fn new() -> Self {
        CallTree {
            nodes: Vec::new(),
            state: CallTreeState::Default,
            active_depth: None,
            deepest_challenge: H256::zero(),
            sec_deepest_challenge: H256::zero(),
            block_number: 0,
        }
    }

    fn add_node(&mut self, node: Node) {
        // Check if a node with the same challenge_id already exists
        if self.has_node_with_challenge_id(node.challenge_id) {
            return;
        }

        self.nodes.push(node);
        self.update_state();
    }

    pub fn add_response(&mut self, mut node: Node) {
        // Check call_tree state
        if self.state != CallTreeState::Challenging {
            return;
        }

        for n in &mut self.nodes {
            if n.challenge_id == node.challenge_id {
                n.state = node.state;
                break;
            }
        }

        self.update_state();
    }

    fn update_state(&mut self) {
        // Ignore all other states as they are the end states

        // 1. Find the nodes with the maximum and second maximum depth that are in Challenging state
        let mut max_depth: Option<u64> = None;
        let mut sec_max_depth: Option<u64> = None;
        let mut max_depth_challenge_id = H256::zero();
        let mut sec_max_depth_challenge_id = H256::zero();
        let mut has_challenging_node = false;
        let mut has_punished_node = false;

        // Traverse all nodes to find the maximum and second maximum depth nodes in Challenging state
        for node in &self.nodes {
            if node.state == NodeState::Punished {
                has_punished_node = true;
            }
            if node.state == NodeState::Challenging {
                has_challenging_node = true;
                let depth = node.call_depth;

                if let Some(current_max) = max_depth {
                    if depth > current_max {
                        // Current node depth is greater than max depth, update max and second max depth
                        sec_max_depth = max_depth;
                        sec_max_depth_challenge_id = max_depth_challenge_id;
                        max_depth = Some(depth);
                        max_depth_challenge_id = node.challenge_id;
                    } else if let Some(current_sec_max) = sec_max_depth {
                        if depth > current_sec_max {
                            // Current node depth is greater than second max depth, update second max depth
                            sec_max_depth = Some(depth);
                            sec_max_depth_challenge_id = node.challenge_id;
                        }
                    } else if depth < current_max {
                        // Current node depth is less than max depth, but second max depth is not set
                        sec_max_depth = Some(depth);
                        sec_max_depth_challenge_id = node.challenge_id;
                    }
                } else {
                    // First challenging node
                    max_depth = Some(depth);
                    max_depth_challenge_id = node.challenge_id;
                }
            }
        }
        // 3. If there is a punished node, set related fields to default values, state to punished
        if has_punished_node {
            self.active_depth = None;
            self.deepest_challenge = H256::zero();
            self.sec_deepest_challenge = H256::zero();
            self.state = CallTreeState::Punished;
            return;
        }

        // 2. If there are no challenging nodes, set related fields to default values, state to responsed
        if !has_challenging_node {
            self.active_depth = None;
            self.deepest_challenge = H256::zero();
            self.sec_deepest_challenge = H256::zero();
            self.state = CallTreeState::Responsed;
            return;
        }

        // Update call_tree fields
        self.active_depth = max_depth;
        self.deepest_challenge = max_depth_challenge_id;
        self.sec_deepest_challenge = sec_max_depth_challenge_id;
        self.state = CallTreeState::Challenging;

        // 3. Find the node with the maximum block_number that is in Responsed state
        let mut max_response_block_number: Option<u64> = None;
        let mut max_response_depth: Option<u64> = None;

        for node in &self.nodes {
            if node.state == NodeState::Responsed {
                if let Some(current_max) = max_response_block_number {
                    if node.block_number > current_max {
                        max_response_block_number = Some(node.block_number);
                        max_response_depth = Some(node.call_depth);
                    }
                } else {
                    max_response_block_number = Some(node.block_number);
                    max_response_depth = Some(node.call_depth);
                }
            }
        }

        // 4. If there is a responsed node with depth greater than the max depth challenge node, adjust timeout
        if let (Some(response_block), Some(response_depth), Some(max_challenge_depth)) =
            (max_response_block_number, max_response_depth, max_depth)
        {
            if response_depth > max_challenge_depth {
                // Find the max depth challenge node and adjust its timeout
                for node in &mut self.nodes {
                    if node.state == NodeState::Challenging
                        && node.call_depth == max_challenge_depth
                    {
                        let new_timeout = response_block + 12;
                        if node.timeout < new_timeout {
                            node.timeout = new_timeout;
                        }
                    }
                }
            }
        }
    }

    // Check if a node with the same challenge_id exists
    fn has_node_with_challenge_id(&self, challenge_id: H256) -> bool {
        for node in &self.nodes {
            if node.challenge_id == challenge_id {
                return true;
            }
        }
        false
    }

    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }

    fn from_json(json_string: &str) -> CallTree {
        serde_json::from_str(json_string).unwrap()
    }
}

impl Node {
    fn new() -> Self {
        Node {
            challenge_id: H256::zero(),
            root_id: H256::zero(),
            timeout: 0,
            caller: H160::zero(),
            callee: None,
            call_depth: 0,
            block_number: 0,
            state: NodeState::Challenging,
        }
    }

    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }

    fn from_json(json_string: &str) -> Node {
        serde_json::from_str(json_string).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert() {
        let mut root_node = Node {
            challenge_id: H256::random(),
            root_id: H256::random(),
            timeout: 6,
            caller: H160::random(),
            callee: Some(H160::random()),
            call_depth: 0,
            block_number: 0,
            state: NodeState::Default,
        };

        let test_node_info2 = NodeInfo {
            challenge_id: H256::random(),
            root_id: H256::random(),
            timeout: 8,
            caller: H160::random(),
            callee: Some(H160::random()),
            call_depth: 1,
            state: NodeState::Default,
        };
        let new_node_info = NodeInfo {
            challenge_id: test_node_info2.challenge_id,
            root_id: test_node_info2.root_id,
            timeout: test_node_info2.timeout,
            caller: test_node_info2.caller,
            callee: test_node_info2.callee,
            call_depth: test_node_info2.call_depth,
            state: test_node_info2.state,
        };

        root_node.insert(new_node_info.clone());

        assert_eq!(root_node.children.len(), 1);
        let inserted_node = &root_node.children[0];
        assert_eq!(inserted_node.challenge_id, test_node_info2.challenge_id);
        assert_eq!(inserted_node.root_id, test_node_info2.root_id);
        assert_eq!(inserted_node.timeout, test_node_info2.timeout);
        assert_eq!(inserted_node.caller, test_node_info2.caller);
        assert_eq!(inserted_node.callee, test_node_info2.callee);
        assert_eq!(inserted_node.call_depth, test_node_info2.call_depth);
    }

    #[test]
    fn test_serde() {
        let root_node = Node {
            challenge_id: H256::random(),
            root_id: H256::random(),
            timeout: 8,
            caller: H160::random(),
            callee: Some(H160::random()),
            call_depth: 0,
            block_number: 0,
            state: NodeState::Default,
        };

        let json_string = serde_json::to_string(&root_node).expect("Failed to serialize to JSON");
        println!("Json String: , {:?}", json_string);
    }
}

lazy_static! {
    static ref CALL_TREE_MAP: Mutex<HashMap<H256, CallTree>> = Mutex::new(HashMap::new());
}

/// List all call trees stored in CALL_TREE_MAP
///
/// Returns a HashMap with root_id as key and CallTree struct as value
pub fn list_all_call_trees() -> HashMap<H256, CallTree> {
    let call_tree_map = CALL_TREE_MAP.lock();
    let mut result = HashMap::new();

    for (root_id, call_tree) in call_tree_map.iter() {
        // Directly clone the CallTree struct
        result.insert(*root_id, call_tree.clone());
    }

    result
}

// This function handles a challenge by updating the call tree map and the state of the nodes involved in the challenge.
pub fn handle_challenge(pom: PoM, block_number: u64) {
    if pom.state != NodeState::Challenging {
        return;
    }
    // Lock call tree map to ensure thread safety
    let mut call_tree_map = CALL_TREE_MAP.lock();
    let root_id = pom.root_id;

    let node_pom = pom.clone();
    let node = Node {
        challenge_id: node_pom.challenge_id,
        root_id: node_pom.root_id,
        timeout: node_pom.timeout,
        caller: node_pom.caller,
        callee: node_pom.callee,
        call_depth: node_pom.call_depth,
        block_number: block_number,
        state: node_pom.state,
    };

    // Check if a call_tree with the corresponding root_id exists in CALL_TREE_MAP
    if let Some(call_tree) = call_tree_map.get_mut(&root_id) {
        // Convert pom to Node and insert it into call_tree
        // Create new node

        // Insert the new node into call_tree
        call_tree.add_node(node);
    } else {
        // If it doesn't exist, create a new call_tree
        let mut call_tree = CallTree::new();

        // Convert pom to Node and insert it into call_tree
        call_tree.add_node(node);

        // Update call_tree state
        call_tree_map.insert(root_id, call_tree);
    }

    // Check if the callee of the current pom is itself
    if pom.callee.unwrap_or_default() == crate::config::mp_NODE_ADDR {
        let mut response_pom = pom.clone();
        // Build and send a response_pom back to the chain
        response_pom.state = NodeState::Responsed;
        tokio::spawn(async {
            crate::l1_helper::update_challenge_bytes(
                crate::config::mp_NODE_ADDR.to_string(),
                response_pom,
                Vec::new(), // TODO: feed a valide signature
            )
            .await
            .unwrap();
        });
    }

    drop(call_tree_map);
}

// This function handles the response of a PoM (Proof of Membership)
pub fn handle_response(pom: PoM, block_number: u64) {
    if pom.state != NodeState::Responsed {
        return;
    }

    // Lock call tree map
    let mut call_tree_map = CALL_TREE_MAP.lock();
    // Get root_id
    let root_id = pom.root_id;

    // Check if a call_tree with the corresponding root_id exists in CALL_TREE_MAP
    if let Some(call_tree) = call_tree_map.get_mut(&root_id) {
        let node = Node {
            challenge_id: pom.challenge_id,
            root_id: pom.root_id,
            timeout: pom.timeout,
            caller: pom.caller,
            callee: pom.callee,
            call_depth: pom.call_depth,
            block_number: block_number,
            state: pom.state,
        };

        call_tree.add_response(node);
    }

    drop(call_tree_map);
}

// This function checks if a PoM has timed out and punishes it if it has
pub fn check_timeout_and_punish(pom: PoM, block_number: u64) {
    // Lock call tree map
    let mut call_tree_map = CALL_TREE_MAP.lock();
    // Get root_id
    let root_id = pom.root_id;

    // Check if a call_tree with the corresponding root_id exists in CALL_TREE_MAP
    if let Some(call_tree) = call_tree_map.get_mut(&root_id) {
        // Use the passed block height instead of fetching it again
        // This ensures we use the actual block height when the event occurred

        if call_tree.state != CallTreeState::Challenging {
            return;
        }

        // Use the deepest_challenge field in call_tree to find the deepest challenge node
        if call_tree.deepest_challenge != H256::zero() {
            // Directly use the H256 type deepest_challenge
            let challenge_id = call_tree.deepest_challenge;

            // Find the corresponding node
            for node in &mut call_tree.nodes {
                if node.challenge_id == challenge_id && node.state == NodeState::Challenging {
                    if node.timeout <= block_number {
                        // Update node state to punished
                        node.state = NodeState::Punished;

                        // Print punishment information
                        println!("Punish {:?}", node.challenge_id);

                        // TODO: send a punish transaction to L1

                        // Update call_tree's active_depth
                        call_tree.update_state();
                    }
                    break;
                }
            }
        }
    }

    drop(call_tree_map);
}
