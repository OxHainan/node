use serde::{Deserialize, Serialize};
use std::fs;
use web3::{
    contract::{Contract, Options},
    types::Address,
};

use crate::{config::*, model::PoM};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub tx_id: Vec<u8>,
    pub nonce: u128,
    pub gas_price: u128,
    pub gas_limit: u128,
    pub to: [u8; 20],
    pub value: u128,
    pub input: Vec<u8>,
    pub v: u8,
    pub r: [u8; 32],
    pub s: [u8; 32],
    pub chain_id: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChallengeData {
    pub challenge_id: [u8; 32],
    pub request_id: [u8; 32],
    pub tx: Transaction,
    pub timeout: u64,
    pub caller: [u8; 20],
    pub callee: Option<[u8; 20]>,
    pub call_depth: u64,
    pub state: crate::call_tree::NodeState,
}
impl ChallengeData {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }

    pub fn from_json(json_string: &str) -> ChallengeData {
        serde_json::from_str(json_string).unwrap()
    }
}

// #[tokio::test]
async fn manual_deploy_pom() -> Result<(), Box<dyn std::error::Error>> {
    let bytecode = fs::read_to_string(mp_BYTECODE_BIN).expect("Failed to read bytecode file");
    let bytecode_vec = hex::decode(bytecode).unwrap();

    let web3 = web3::Web3::new(web3::transports::WebSocket::new(ETH_ADDR).await.unwrap());

    let accounts = web3.eth().accounts().await?;
    let account = accounts[0];

    let deploy_transaction = web3::types::TransactionRequest {
        from: account,
        data: Some(bytecode_vec.into()),
        gas: Some(4_000_000.into()),
        ..Default::default()
    };

    let transaction_hash = web3.eth().send_transaction(deploy_transaction).await?;

    loop {
        match web3.eth().transaction_receipt(transaction_hash).await? {
            Some(receipt) => {
                println!(
                    "Contract mp deployed at address: {}",
                    receipt.contract_address.unwrap()
                );
                break;
            }
            None => {
                println!("Waiting for mp deployment confirmation...");
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        }
    }

    Ok(())
}

// #[tokio::test]
async fn manual_register_tee() -> Result<(), Box<dyn std::error::Error>> {
    let peer_id = String::from("peeridtest");
    let quote_size: u32 = 0;
    let quote_buf: Vec<u8> = vec![0x68, 0x65, 0x6c, 0x6c, 0x6f];
    let sup_size: u32 = 0;
    let sup_buf: Vec<u8> = vec![0x68, 0x65, 0x6c, 0x6c, 0x6f];
    let tee_public_key = String::from("teePublicKeytest");
    let p2p_connect_info = String::from("p2pConnectInfotest");
    let app_addr = String::from("appAddrTest");

    register_tee(
        peer_id,
        quote_size,
        quote_buf,
        sup_size,
        sup_buf,
        tee_public_key,
        p2p_connect_info,
        app_addr,
    )
    .await?;
    Ok(())
}
pub async fn register_tee(
    peer_id: String,
    quote_size: u32,
    quote_buf: Vec<u8>,
    sup_size: u32,
    sup_buf: Vec<u8>,
    tee_public_key: String,
    p2p_connect_info: String,
    app_addr: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let web3 = web3::Web3::new(web3::transports::WebSocket::new(ETH_ADDR).await.unwrap());
    let accounts = web3.eth().accounts().await?;
    let account = accounts[0];

    let contract_address: Address = mp_CONTRACT_L1_ADDR.parse()?;
    let abi = fs::read_to_string(mp_BYTECODE_ABI).expect("Failed to read ABI file");
    let contract = Contract::from_json(web3.eth(), contract_address, abi.as_bytes())?;
    let result = contract
        .call(
            "registerTEE",
            (
                peer_id,
                quote_size,
                quote_buf,
                sup_size,
                sup_buf,
                tee_public_key,
                p2p_connect_info,
                app_addr,
            ),
            account,
            Options::default(),
        )
        .await?;

    println!("Transaction hash: {:?}", result);

    Ok(())
}

// #[tokio::test]
async fn manual_register_api() -> Result<(), Box<dyn std::error::Error>> {
    let peer_id = String::from("peeridtest");
    let app_addr = String::from("appaddrtest");
    let method = String::from("methodtest");
    let timeout: u32 = 6;
    register_api(peer_id, app_addr, method, timeout).await?;
    Ok(())
}
pub async fn register_api(
    peer_id: String,
    app_addr: String,
    method: String,
    timeout: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let web3 = web3::Web3::new(web3::transports::WebSocket::new(ETH_ADDR).await.unwrap());
    let accounts = web3.eth().accounts().await?;
    let account = accounts[0];

    let contract_address: Address = mp_CONTRACT_L1_ADDR.parse()?;
    let abi = fs::read_to_string(mp_BYTECODE_ABI).expect("Failed to read ABI file");
    let contract = Contract::from_json(web3.eth(), contract_address, abi.as_bytes())?;
    let result = contract
        .call(
            "registerApi",
            (peer_id, app_addr, method, timeout),
            account,
            Options::default(),
        )
        .await?;

    println!("Transaction hash: {:?}", result);

    Ok(())
}

#[tokio::test]
async fn test_new_challenge() -> Result<(), Box<dyn std::error::Error>> {
    let challenge_data = ChallengeData {
        challenge_id: [0; 32],
        request_id: [0; 32],
        tx: Transaction {
            tx_id: vec![0; 32],
            nonce: 0,
            gas_price: 0,
            gas_limit: 0,
            to: [0; 20],
            value: 0,
            input: vec![0; 32],
            v: 0,
            r: [0; 32],
            s: [0; 32],
            chain_id: 0,
        },
        timeout: 0,
        caller: [0; 20],
        callee: Some([0; 20]),
        call_depth: 0,
        state: crate::fsm::State::Default,
    };
    // new_challenge(challenge_data).await?;
    Ok(())
}
pub async fn new_challenge(
    challenge_data: ChallengeData,
) -> Result<(), Box<dyn std::error::Error>> {
    let web3 = web3::Web3::new(web3::transports::WebSocket::new(ETH_ADDR).await.unwrap());
    let accounts = web3.eth().accounts().await?;
    let account = accounts[0];

    let contract_address: Address = mp_CONTRACT_L1_ADDR.parse()?;
    let abi = fs::read_to_string(mp_BYTECODE_ABI).expect("Failed to read ABI file");
    let contract = Contract::from_json(web3.eth(), contract_address, abi.as_bytes())?;

    let result = contract
        .call(
            "newChallenge",
            (challenge_data.to_json(),),
            account,
            Options::default(),
        )
        .await?;

    println!("Transaction hash: {:?}", result);

    Ok(())
}

#[tokio::test]
async fn test_update_challenge_bytes() -> Result<(), Box<dyn std::error::Error>> {
    let a = ethereum::EIP1559Transaction {
        chain_id: 0,
        nonce: ethereum_types::U256::zero(),
        max_priority_fee_per_gas: ethereum_types::U256::zero(),
        max_fee_per_gas: ethereum_types::U256::zero(),
        gas_limit: ethereum_types::U256::zero(),
        action: ethereum::TransactionAction::Create,
        value: ethereum_types::U256::zero(),
        input: "test".as_bytes().to_vec(),
        access_list: Vec::new(),
        odd_y_parity: false,
        r: ethereum_types::H256::zero(),
        s: ethereum_types::H256::zero(),
    };
    let pom = PoM {
        challenge_id: ethereum_types::H256([1u8; 32]),
        root_id: ethereum_types::H256::zero(),
        tx: ethereum::TransactionV2::EIP1559(a),
        timeout: 6,
        caller: ethereum_types::H160::zero(),
        callee: Some(ethereum_types::H160::zero()),
        call_depth: 0,
        state: crate::fsm::State::Default,
    };
    // update_challenge_bytes(String::from("peeridtest"), pom, Vec::new()).await?;
    Ok(())
}
pub async fn update_challenge_bytes(
    peer_id: String,
    pom: PoM,
    sig: Vec<u8>,
) -> Result<(), Box<dyn std::error::Error>> {
    let web3 = web3::Web3::new(web3::transports::WebSocket::new(ETH_ADDR).await.unwrap());
    let accounts = web3.eth().accounts().await?;
    let account = accounts[0];

    let contract_address: Address = mp_CONTRACT_L1_ADDR.parse()?;
    let abi = fs::read_to_string(mp_BYTECODE_ABI).expect("Failed to read ABI file");
    let contract = Contract::from_json(web3.eth(), contract_address, abi.as_bytes())?;
    let options = Options {
        gas: Some(4_000_000.into()),
        ..Default::default()
    };
    println!(
        "pom:{:?}, bytes:{:?}",
        pom.to_json(),
        pom.to_json().into_bytes()
    );

    let result = contract
        .call(
            "updateChallengeBytes",
            (
                peer_id,
                pom.challenge_id.as_bytes().to_vec(),
                pom.to_json().into_bytes(),
                sig,
            ),
            account,
            options,
        )
        .await?;

    println!("Transaction hash: {:?}", result);

    Ok(())
}

pub async fn get_block_number() -> u64 {
    let web3 = web3::Web3::new(web3::transports::WebSocket::new(ETH_ADDR).await.unwrap());
    let block_number = web3.eth().block_number().await.unwrap();
    block_number.as_u64()
}
