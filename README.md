
## Key Features

- **Docker-based Smart Contracts**: Write smart contracts as standard Web2 applications using any language or framework
- **Decentralized Consensus**: Raft-based consensus algorithm ensures transaction integrity across nodes
- **Web2-like Developer Experience**: Familiar APIs, development patterns, and tooling
- **Automatic State Synchronization**: All state changes are automatically tracked and synchronized
- **Proxied External Requests**: Blockchain nodes proxy external requests to ensure deterministic execution
- **Scheduled Tasks**: Support for blockchain-tracked scheduled tasks
- **Comprehensive SDK**: Easy-to-use SDK for interacting with the blockchain

## Architecture

The architecture consists of several key components that work together to provide a robust blockchain platform:

### Core components

```
├── node/                # Blockchain node implementation
│   ├── crates/
│   │   ├── common/      # Common types and utilities
│   │   ├── consensus/   # Consensus implementation (Raft)
│   │   ├── mempool/     # Transaction pool
│   │   ├── compute/     # Execution environment (didn't use it now, consider to merge with executor in the future)
│   │   ├── state/       # State management
│   │   ├── network/     # P2P networking
│   │   ├── executor/    # transaction execution
│   │   └── sdk/         # Client SDK
│   ├── src/             # Node entry point and core logic
│   └── examples/        # Example applications
│       └── counter/     # Simple counter example
├── docs/                # Documentation
└── ...
```

1. **Transaction Pool (Mempool)**: 
   - Manages incoming transactions and prepares them for consensus
   - Validates transaction format and signature
   - Maintains transaction status tracking
   - Acts as the entry point for all transactions in the system

2. **Consensus Layer**: 
   - Implements Raft consensus to ensure transaction ordering and replication
   - Provides leader election and log replication
   - Ensures all nodes maintain the same transaction history
   - Generates a deterministic transaction sequence

3. **Computation Layer**: 
   - Executes transactions in Docker containers
   - Provides isolation between different smart contracts
   - Allows for any programming language to be used for contract development
   - Ensures deterministic execution

4. **State Storage**: 
   - Maintains application state across the blockchain
   - Manages state transitions and validation
   - Provides querying capabilities
   - Ensures data consistency

5. **P2P Network**: 
   - Handles node discovery and communication
   - Manages peer connections and heartbeats
   - Facilitates transaction propagation
   - Ensures network resilience

6. **Executor Layer**:
   - Executes smart contract code in a controlled environment
   - Processes transactions through a multi-worker thread pool
   - Tracks state changes during execution
   - Maintains execution metadata for blockchain integration
   - Provides isolation between contract modules




### Transaction Management

Transactions in the node represent all operations that modify the blockchain state. They are unified under a common `Transaction` structure with different types:

- **API Request**: External API calls to smart contracts
- **State Change**: Direct state modifications
- **Scheduled Task**: Time-based operations

The node implements an **"execute-then-consensus"** model, which differs from traditional blockchains:
- Transactions are executed before going through consensus
- Execution results become part of the consensus data
- This approach enables validation of both transactions and their outcomes
- It creates efficiency by preventing invalid transactions from consuming consensus resources

Each transaction is uniquely identified by a UUID and contains:
- **ID**: A unique identifier (UUID)
- **Type**: The transaction type
- **Payload**: Operation-specific data
- **Timestamp**: Creation time
- **Sender**: Optional sender identifier
- **Log Index**: Position in the consensus log

### Transaction Execution System: Mempool-Executor-Consensus Interface

The node platform implements a streamlined execution model that bridges traditional Web2 development with blockchain capabilities through carefully designed interfaces between its core components:

#### 1. Input, Output, and State Diff Handling

The transaction flow follows this pattern:
- **Mempool** receives and validates transaction requests through its API layer
- **Consensus** orders transactions and ensures all nodes process them identically
- **Executor** processes the transactions and captures state changes:
  - Receives `ExecutionRequest` containing module ID, handler name, and input parameters
  - Executes the appropriate contract handler with the input parameters
  - Captures all state modifications during execution
  - Returns `ExecutionResult` containing output data and `StateDiff` records
  - State changes are tracked as key-value insertions, updates, and deletions
- Results flow back to **Mempool** which makes them available via APIs

This design separates transaction acceptance (mempool), ordering (consensus), and execution (executor) while maintaining atomic state transitions and consistent results across all nodes.

#### 2. Web2-Friendly Development Experience

The architecture is specifically designed to make blockchain development feel familiar to Web2 developers:

- **JSON-Based Interface**: Contract handlers receive JSON input and return JSON output, similar to REST APIs
- **Function-Based Programming Model**: Developers implement simple handler functions that process input and return output
- **Abstracted State Management**: State access is provided through simple read/write interfaces
- **No Blockchain Knowledge Required**: Developers can focus on business logic without understanding consensus or blockchain internals
- **Familiar Programming Paradigms**: Handlers are implemented as closures/functions with standard control flow

For example, a typical handler looks like this:

```rust
// A simple counter increment handler with detailed comments
Box::new(|input| {
    // Parse JSON input, defaulting to 1 if "amount" is not provided
    // This is just like parsing a JSON request body in a web API
    let amount = input.get("amount")
        .and_then(|v| v.as_u64())  // Convert to u64 if possible
        .unwrap_or(1);             // Default to 1 if missing or invalid
    
    // Access the state - similar to accessing a database in Web2
    // No need to understand blockchain state management details
    let mut value = state.lock().unwrap();
    
    // Update the state - just like updating a value in memory or database
    // The system automatically tracks this change for the blockchain
    *value += amount;
    
    // Return a JSON response - identical to returning JSON from a REST API
    // Framework handles serialization, consensus, and state updates automatically
    Ok(serde_json::json!({ "value": *value }))
})
```

For more complex applications, the pattern remains just as simple:

```rust
// User registration handler example
Box::new(|input| {
    // Extract user information from JSON input
    let username = input.get("username")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::InvalidInput("Username is required".to_string()))?;
    
    let email = input.get("email")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::InvalidInput("Email is required".to_string()))?;
    
    // Check if user already exists - similar to a database lookup
    let users = state.get_collection("users")?;
    if users.contains_key(username)? {
        return Err(Error::AlreadyExists("User already exists".to_string()));
    }
    
    // Create new user object
    let new_user = serde_json::json!({
        "username": username,
        "email": email,
        "created_at": chrono::Utc::now().to_rfc3339(),
        "status": "active"
    });
    
    // Store in state - framework tracks this write operation automatically
    users.insert(username, &new_user)?;
    
    // Return success response with created user data
    Ok(serde_json::json!({
        "status": "success",
        "message": "User registered successfully",
        "user": new_user
    }))
})
```

This approach allows developers to write blockchain applications using familiar programming patterns similar to web services or REST APIs. The node system automatically handles:

1. **State Tracking**: All reads and writes are tracked without developer intervention
2. **Transaction Processing**: The framework handles transaction submission and confirmation
3. **Consensus**: Developers don't need to think about blockchain ordering or validation
4. **Serialization**: JSON handling works just like in a typical web framework

Developers familiar with building REST APIs or microservices can immediately be productive with node without learning blockchain-specific concepts or programming patterns.

### Transaction Lifecycle

Transactions in the node follow a well-defined lifecycle:

1. **Submission**: 
   - Client submits a transaction through the SDK or API
   - Transaction is assigned a unique UUID
   - The transaction is validated for correct format and required fields

2. **Mempool Processing**:
   - Transaction is added to the `pending_transactions` queue in the `BasicTransactionPool`
   - The transaction is stored in the `transaction_map` with a status of "pending"
   - A transaction response is created and stored in the `transaction_results` map
   - The client receives a transaction ID for status tracking

3. **Execution**:
   - Transaction is processed by the execution environment
   - The transaction is executed according to its type (state change, API request, etc.) 
   - Execution results are captured and returned for consensus processing
   - This is a key distinction - node uses an "execute-then-consensus" model

4. **Consensus Processing**:
   - The mempool submits the executed transaction along with its results to the consensus engine
   - In the Raft implementation, the leader node adds the transaction to its log
   - The transaction is replicated to follower nodes
   - Once a majority of nodes have acknowledged the transaction, it's considered committed

5. **Result Processing**:
   - The final transaction result is passed to the mempool via the `update_transaction_result` method
   - The result is stored in the `transaction_results` map
   - The transaction status is updated to "success" or "failed"
   - The transaction is removed from the `transaction_map` after processing
   - This completes the "execute-then-consensus" flow

6. **Client Notification**:
   - Clients can query the transaction status using the transaction ID
   - When a transaction is completed, its final result is made available
   - SDKs provide helper methods like `wait_for_transaction` to simplify this process


### Transaction Flow in Detail

Let's trace a transaction through the system:

1. **Client Submission**:
   ```rust
   // Client code
   let transaction = TransactionBuilder::new(TransactionType::StateChange)
       .payload(&json!({ ... }))
       .build();
   
   let tx_id = client.submit_transaction(transaction).await?;
   ```

2. **API Reception**:
   - The JSON-RPC server receives the request in `submit_transaction`
   - It validates the transaction format and fields
   - The transaction is forwarded to the transaction pool

3. **Mempool Processing**:
   ```rust
   // In BasicTransactionPool::submit_transaction
   let tx_id = transaction.id.to_string();
   
   // Create response and store in results map
   let response = TransactionResponse { 
       tx_id: tx_id.clone(),
       status: "pending".to_string(),
       result: None,
       error: None
   };
   
   // Store in transaction map for tracking
   transaction_map.insert(tx_id.clone(), transaction.clone());
   
   // Add to pending queue
   self.pending_transactions.lock().await.push_back(transaction);
   ```

4. **Execution**:
   ```rust
   // In mempool's process_transactions method
   let tx = self.pending_transactions.lock().await.pop_front().unwrap();
   
   // Execute the transaction first
   let result = executor.execute_transaction(tx.clone()).await?;
   
   // Then submit to consensus with the result
   self.consensus_engine.submit_transaction_with_result(tx.clone(), result).await?;
   ```

5. **Consensus Processing**:
   - Leader receives transaction and its result and appends to log
   - Transaction is replicated to followers
   - Once majority-confirmed, transaction is committed
   - Transaction is sent to the confirmed transaction channel with its result


This "execute-then-consensus" approach distinguishes node from traditional blockchains that typically use a "consensus-then-execute" model. By executing transactions before consensus:

1. Nodes can validate transaction results as part of consensus
2. Failed transactions can be identified before committing to the blockchain
3. Execution results are part of what nodes reach consensus on
4. The system maintains a record of both transactions and their outcomes

## Getting Started

### Prerequisites

- Rust 1.70+ 

### Installation

```bash
# Clone the repository
git clone https://github.com/oxhainan/node.git
cd node

# Build the project
cargo build --release --bin mp-node

```

## Running with simulator mode
if you want to run the node in simulator mode, you can start with tdx simulator.

```bash
# Start cvm simulator
docker pull phalanetwork/tappd-simulator:latest
docker run --rm -p 8090:8090 phalanetwork/tappd-simulator:latest
```

## Run the node

```bash
cargo run --release --bin mp-node -- --config config.toml --with-rest-api --log-level info 
```

## Running Tests

node and its examples include tests that can be run without a running node:

running test you must stop the node first.

```bash
# running web_style example
./scripts/test_node_workflow.sh

# running openai example
./scripts/test_openai_workflow.sh
```

