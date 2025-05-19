use crate::{config::*, model::PoM};
use ethabi::{Event, EventParam, ParamType, RawLog};
use hex_literal::hex;
use web3::{
    ethabi,
    futures::StreamExt,
    types::{Address, FilterBuilder, Log, H256},
};

// async function to listen to events
async fn listen_events() {
    // create a new web3 instance using a websocket connection
    let web3 = web3::Web3::new(web3::transports::WebSocket::new(ETH_ADDR).await.unwrap());
    // parse the contract address
    let contract_address: Address = mp_CONTRACT_L1_ADDR.parse().unwrap();

    // create a filter to listen to logs from the contract address
    let filter = FilterBuilder::default()
        .address(vec![contract_address])
        .topics(
            Some(vec![H256::from_slice(&hex!(
                "b99104c672a3595b68bee7f956d813e12ceef4312cf778729dcbb860c3473e5a"
            ))]),
            None,
            None,
            None,
        )
        .build();
    // subscribe to the logs
    let mut subscription = web3.eth_subscribe().subscribe_logs(filter).await;
    println!(
        "Subscribed to logs with subscription id: {:?}",
        subscription
    );

    // loop through the logs
    while let Some(log) = subscription.as_mut().expect("subscription").next().await {
        match log {
            Ok(log) => handle_log(log),
            Err(e) => eprintln!("Error fetching log: {}", e),
        }
    }
}

// Function to handle a log
fn handle_log(log: Log) {
    // Create an event with the name "ChallengeEvent" and an input parameter "data" of type bytes
    let event = Event {
        name: "ChallengeEvent".to_owned(),
        inputs: vec![EventParam {
            name: "data".to_owned(),
            kind: ParamType::Bytes,
            indexed: false,
        }],
        anonymous: false,
    };

    // Extract block number from the log
    let block_number = log.block_number.map_or(0, |bn| bn.as_u64());
    println!("Log from block number: {}", block_number);

    // Create a raw log with the topics and data from the input log
    let raw_log = RawLog {
        topics: log.topics,
        data: log.data.0,
    };
    // Parse the raw log using the event
    let decoded_logs = event.parse_log(raw_log).expect("Failed to parse log");
    // Convert the value of the first parameter to bytes and then to a string
    let json_bytes: Vec<u8> = decoded_logs.params[0].value.clone().into_bytes().unwrap();
    let json_string = String::from_utf8(json_bytes).unwrap();
    // Parse the JSON string into a PoM
    let pom = PoM::from_json(json_string.as_ref());
    // Print the received PoM
    println!("Recived PoM: {:?}", pom.clone());
    // If the PoM state is "Challenging", call the handle_challenge function
    if pom.state == crate::call_tree::NodeState::Challenging {
        crate::call_tree::handle_challenge(pom.clone(), block_number);
    // If the PoM state is "Responsed", call the handle_response function
    } else if pom.state == crate::call_tree::NodeState::Responsed {
        crate::call_tree::handle_response(pom.clone(), block_number);
    }
    // Check timeout from the root node in pom
    crate::call_tree::check_timeout_and_punish(pom, block_number);
}

pub async fn manual_listen_events() -> Result<(), Box<dyn std::error::Error>> {
    listen_events().await;
    Ok(())
}

#[tokio::test]
async fn calc_topic() -> Result<(), Box<dyn std::error::Error>> {
    let event = Event {
        name: "ChallengeEvent".to_string(),
        inputs: vec![EventParam {
            name: "data".to_string(),
            kind: ParamType::Bytes,
            indexed: false,
        }],
        anonymous: false,
    };

    let topic = event.signature();
    println!("Event topic: {:?}", topic);

    Ok(())
}
