use ethereum_types::{H160, H256, U256};
use mp_pom::{
    // Remove unused imports
    // event_handler,
    call_tree::NodeState,
    l1_helper,
    model::{PoM, Transaction},
};
use std::str::FromStr;
use tokio;
// Add signature related dependencies
use secp256k1::{Message, PublicKey, Secp256k1, SecretKey};
// Add event listening related dependencies
use std::time::Duration;
use web3::{
    contract::{Contract, Options},
    signing::keccak256, // Fix: import the correct keccak256 function
    types::{BlockNumber, FilterBuilder, Log},
};

#[tokio::test]
#[ignore = "需要合约已经部署并配置好"]
async fn test_register_tee_and_challenge() -> Result<(), Box<dyn std::error::Error>> {
    // First create a consistent key pair
    let secp = Secp256k1::new();
    let secret_key = SecretKey::from_slice(&[0x01; 32]).expect("32 bytes, within curve order");
    let public_key = PublicKey::from_secret_key(&secp, &secret_key);

    // Derive Ethereum address from public key
    let hash = keccak256(&public_key.serialize_uncompressed()[1..]);
    let eth_address = format!("0x{}", hex::encode(&hash[12..]));

    // 1. Register TEE node, using the derived Ethereum address as tee_public_key
    let peer_id = eth_address.clone();
    let quote_size: u32 = 100;
    let quote_buf: Vec<u8> = vec![1, 2, 3, 4, 5];
    let sup_size: u32 = 50;
    let sup_buf: Vec<u8> = vec![6, 7, 8, 9, 10];
    let tee_public_key = eth_address; // Use derived address
    let p2p_connect_info = String::from("ip4/127.0.0.1/tcp/4242/p2p/QmTest");
    let app_addr = String::from("test_app_addr");

    // Call registerTEE function
    l1_helper::register_tee(
        peer_id.clone(),
        quote_size,
        quote_buf,
        sup_size,
        sup_buf,
        tee_public_key.clone(),
        p2p_connect_info,
        app_addr.clone(),
    )
    .await?;

    // 2. Then register API - using the same peer_id and app_addr
    let method = String::from("test_method");
    let timeout: u32 = 300;

    // Call registerApi function
    l1_helper::register_api(peer_id.clone(), app_addr.clone(), method, timeout).await?;

    // 3. Finally create a challenge
    let tx = ethereum::LegacyTransaction {
        nonce: U256::from(1),
        gas_price: U256::from(1000000000),
        gas_limit: U256::from(21000),
        action: ethereum::TransactionAction::Call(
            H160::from_str(&tee_public_key).map_err(|e| e.to_string())?,
        ),
        value: U256::zero(),
        input: vec![].into(),
        signature: ethereum::TransactionSignature::new(
            27,
            H256::from_slice(&[1u8; 32]),
            H256::from_slice(&[2u8; 32]),
        )
        .ok_or("Failed to create signature")?,
    };

    let pom = PoM {
        root_id: H256::from_slice(&[1u8; 32]),
        challenge_id: H256::from_slice(&[2u8; 32]),
        tx: Transaction::Legacy(tx),
        timeout: 300,
        caller: H160::from_str(&tee_public_key).map_err(|e| e.to_string())?,
        callee: Some(
            H160::from_str("0x0987654321098765432109876543210987654321")
                .map_err(|e| e.to_string())?,
        ),
        call_depth: 0,
        state: NodeState::Challenging,
    };

    // According to the updateChallenge function in the contract, we need to sign the data: abi.encodePacked(appAddr, challengeId, timeout, status, peerId)
    // Create a message matching the contract
    let challenge_id_bytes = pom.challenge_id.as_bytes();
    let status: u32 = 1; // challenging status, explicitly specify type as u32

    // Build message format same as the contract
    let mut message_data = Vec::new();
    message_data.extend_from_slice(app_addr.as_bytes());
    message_data.extend_from_slice(challenge_id_bytes);
    // 修复: 使用pom中的timeout而不是外部变量
    message_data.extend_from_slice(&pom.timeout.to_be_bytes());
    message_data.extend_from_slice(&status.to_be_bytes());
    message_data.extend_from_slice(peer_id.as_bytes());

    // Hash the message
    let hash = keccak256(&message_data);

    // Create message object
    let message = Message::from_slice(&hash).expect("32 bytes");

    // Sign the message with private key - fix here
    let signature = secp.sign_ecdsa_recoverable(&message, &secret_key);

    // Convert signature to byte array - fix here
    let mut sig_bytes = Vec::with_capacity(65);
    let (recovery_id, signature_serialized) = signature.serialize_compact();
    sig_bytes.extend_from_slice(&signature_serialized[..32]); // r
    sig_bytes.extend_from_slice(&signature_serialized[32..]); // s

    // Add recovery ID (v)
    // Fix: use to_i32() method to get integer value
    sig_bytes.push(recovery_id.to_i32() as u8 + 27); // correct v value

    // Set up event listener before calling the contract
    let web3 = web3::Web3::new(web3::transports::WebSocket::new(mp_pom::config::ETH_ADDR).await?);
    let contract_address =
        mp_pom::config::mp_CONTRACT_L1_ADDR.parse::<web3::types::Address>()?;

    // Get current block number, we only listen for events from now on
    let current_block = web3.eth().block_number().await?;

    // Create event filter
    let filter = FilterBuilder::default()
        .address(vec![contract_address])
        .from_block(BlockNumber::Number(current_block))
        .topics(
            Some(vec![web3::types::H256::from_slice(
                // Fix: use the correctly imported keccak256 function
                &keccak256("ChallengeEvent(bytes)".as_bytes())[..],
            )]),
            None,
            None,
            None,
        )
        .build();

    // Call updateChallengeBytes with the same peer_id and valid signature
    l1_helper::update_challenge_bytes(peer_id, pom.clone(), sig_bytes).await?;

    // Wait for transaction confirmation
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Get event logs
    let logs = web3.eth().logs(filter).await?;
    println!("Found {} event logs", logs.len());

    // Parse event logs
    for log in logs {
        println!("Processing log: {:?}", log);
        if let Some(data) = log.data.0.get(..32) {
            // Try to parse event data
            match parse_challenge_event_data(&log.data.0) {
                Ok(parsed_pom) => {
                    println!("Successfully parsed PoM from event:");
                    println!("  Challenge ID: {:?}", parsed_pom.challenge_id);
                    println!("  Root ID: {:?}", parsed_pom.root_id);
                    println!("  Timeout: {}", parsed_pom.timeout);
                    println!("  State: {:?}", parsed_pom.state);

                    // Verify that the parsed PoM matches what we sent
                    assert_eq!(parsed_pom.challenge_id, pom.challenge_id);
                    assert_eq!(parsed_pom.root_id, pom.root_id);
                    assert_eq!(parsed_pom.timeout, pom.timeout);
                    assert_eq!(parsed_pom.state, pom.state);
                }
                Err(e) => {
                    println!("Failed to parse event data: {}", e);
                }
            }
        }
    }

    Ok(())
}

// Function to parse event data
fn parse_challenge_event_data(data: &[u8]) -> Result<PoM, Box<dyn std::error::Error>> {
    // Event data is ABI-encoded bytes, we need to decode the bytes first
    // In Solidity, the ABI encoding of bytes type includes a 32-byte offset, then 32-byte length, then the actual data

    // Skip the first 32 bytes (offset)
    let data = &data[32..];

    // Read the length (32 bytes)
    let length = u32::from_be_bytes([data[28], data[29], data[30], data[31]]) as usize;

    // Read the actual data
    let actual_data = &data[32..(32 + length)];

    // Assume the actual data is a JSON-formatted PoM
    let json_str = std::str::from_utf8(actual_data)?;
    let pom: PoM = serde_json::from_str(json_str)?;

    Ok(pom)
}
