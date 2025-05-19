# mp Node Integrated RESTful API

This module provides a RESTful API interface directly integrated into the mp node. Unlike the previous approach that used a separate gateway module communicating with the node through the SDK (which introduced network overhead), this implementation directly interfaces with the node's internal components for improved performance.

## Architecture Benefits

1. **Performance Improvement**: Direct integration eliminates network overhead between the gateway and node
2. **Simplified Deployment**: No need to deploy and manage a separate gateway service
3. **Reduced Complexity**: Unified codebase with direct access to node components
4. **Improved Resource Utilization**: Shared memory and resources between the node and API

## Components

- **IntegratedRestApi**: Main REST API implementation that handles HTTP requests
- **ApiKeyStore**: Manages API keys and their associated blockchain addresses
- **AdminInterface**: Administrative interface for managing API keys

## Configuration

Add the following section to your `config.toml`:

```toml
[rest_api]
# Path to API key store database
key_store_path = "./api_keys.db"

# Bind address for the REST API
rest_bind_address = "127.0.0.1:3000"

# Bind address for the admin interface
admin_bind_address = "127.0.0.1:3001"

# Transaction timeout in seconds
tx_timeout = 30
```

## Usage

To enable the integrated REST API, start the node with the `--with-rest-api` flag:

```bash
cargo run -- --with-rest-api
```

## API Endpoints

### REST API Endpoints

The REST API provides HTTP access to the mp blockchain functionality. All requests require API key authentication.

### Admin Interface Endpoints

- `POST /api-keys` - Generate a new API key
- `GET /api-keys` - List all API keys
- `DELETE /api-keys/{api-key}` - Revoke an API key
- `GET /health` - Health check

## Authentication

Authentication is done via API keys, which can be provided in one of the following ways:

1. Bearer token in Authorization header: `Authorization: Bearer <api-key>`
2. Custom header: `X-API-Key: <api-key>`
3. Query parameter: `?api_key=<api-key>`

## Implementation Details

This implementation interfaces directly with:

1. **Transaction Pool**: For immediate transaction submission without network overhead
2. **Execution Engine**: For processing requests directly without intermediary network calls
3. **Result Handling**: Uses direct channel communication rather than HTTP requests

The flow of a typical REST API request:

1. HTTP request is received
2. API key is validated
3. Request is converted to transaction payload
4. Transaction is submitted directly to local mempool
5. Execution request is sent directly to executor
6. Results are returned via direct channel communication
7. HTTP response is generated based on execution results
