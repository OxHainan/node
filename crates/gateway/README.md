# mp RESTful Gateway

The mp RESTful Gateway provides a bridge between traditional RESTful APIs and the mp blockchain. It allows web2 applications to interact with the blockchain without modifying their code, simply by redirecting their API requests to the gateway.

## Features

- **RESTful API Compatibility**: Accepts standard HTTP requests and converts them to blockchain transactions
- **API Key Management**: Maps API keys to blockchain addresses for authentication
- **Automatic Nonce Management**: Handles transaction nonces automatically
- **Admin Interface**: Provides an interface for managing API keys
- **Transaction Tracking**: Waits for transaction confirmation and returns results

## Architecture

The gateway consists of three main components:

1. **RESTful Gateway**: Handles incoming HTTP requests, converts them to blockchain transactions, and returns the results
2. **API Key Store**: Manages the mapping between API keys and blockchain addresses
3. **Admin Interface**: Provides an HTTP API for managing API keys

## Installation

```bash
# Build the gateway
cd /path/to/mp/node/crates/gateway
cargo build --release
```

## Configuration

Create a `gateway_config.toml` file with the following structure:

```toml
[gateway]
# URL of the mp node
node_url = "http://127.0.0.1:8545"

# Path to API key store database
key_store_path = "./api_keys.db"

# Bind address for the gateway
gateway_bind_address = "127.0.0.1:3000"

# Bind address for the admin interface
admin_bind_address = "127.0.0.1:3001"

# Transaction timeout in seconds
tx_timeout = 30
```

## Usage

### Starting the Gateway

```bash
# Start the gateway
cargo run --release -- --config gateway_config.toml
```

### Managing API Keys

#### Adding an API Key

```bash
curl -X POST "http://localhost:3001/admin/api-keys" \
  -H "Content-Type: application/json" \
  -d '{"api_key":"your-api-key","address":"0x1234567890abcdef1234567890abcdef12345678"}'
```

#### Listing API Keys

```bash
curl -X GET "http://localhost:3001/admin/api-keys"
```

#### Deleting an API Key

```bash
curl -X DELETE "http://localhost:3001/admin/api-keys/your-api-key"
```

### Making Requests Through the Gateway

Once you have added an API key, you can make requests through the gateway by including the API key in your request:

```bash
# Using the X-API-Key header
curl -X GET "http://localhost:3000/api/your-endpoint" \
  -H "X-API-Key: your-api-key"

# Using the Authorization header
curl -X GET "http://localhost:3000/api/your-endpoint" \
  -H "Authorization: Bearer your-api-key"

# Using a query parameter
curl -X GET "http://localhost:3000/api/your-endpoint?api_key=your-api-key"
```

## Testing

The gateway includes several test scripts to verify its functionality:

```bash
# Run the Bash test script
./scripts/test_gateway.sh

# Run the Python test script
python3 ./scripts/test_gateway.py

# Run the integration test (requires a running mp node)
cargo test --test test_gateway -- --ignored
```

## How It Works

1. **Request Reception**: The gateway receives an HTTP request with an API key
2. **API Key Lookup**: The gateway looks up the blockchain address associated with the API key
3. **Transaction Creation**: The gateway creates a blockchain transaction with the request details
4. **Transaction Submission**: The transaction is submitted to the mp node
5. **Result Retrieval**: The gateway waits for the transaction to be confirmed and retrieves the result
6. **Response Generation**: The gateway converts the transaction result into an HTTP response and returns it to the client

## Integration with Existing Applications

To integrate the gateway with an existing application, simply:

1. Add an API key for your application through the admin interface
2. Update your application's API endpoint to point to the gateway
3. Include the API key in your requests

No other changes to your application code are required!

## License

This project is licensed under the MIT License - see the LICENSE file for details. 