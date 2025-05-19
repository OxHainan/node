#!/bin/bash

# mp Blockchain Workflow Test Script
# This script demonstrates a complete workflow for the mp blockchain:
# 1. Starting a mp node with REST API enabled
# 2. Starting the openai_proxy example separately
# 3. Interacting with the openai_proxy smart contract through the REST API
# 4. Sending multiple API requests and verifying responses

# Set colors for better output readability
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
REST_API_URL="http://localhost:3000"
ADMIN_API_URL="http://localhost:3001"
NODE_DIR="$(dirname "$0")/.." # Parent directory of scripts folder

# Print banner
echo -e "${YELLOW}"
echo "==============================================="
echo "  mp Blockchain Complete Workflow Test"
echo "==============================================="
echo -e "${NC}"

# Function to check if a process is running on specific ports
check_port_usage() {
    if lsof -i :3000,3001 > /dev/null 2>&1; then
        echo -e "${RED}ERROR: Ports 3000 or 3001 are already in use.${NC}"
        echo "Please stop any running mp node instances before starting a new one."
        echo "You can use: lsof -i :3000,3001 to see which processes are using these ports."
        echo "Then use: kill <PID> to terminate the process."
        return 1
    fi
    return 0
}

# Function to start the mp node in the background
start_mp_node() {
    echo -e "${YELLOW}Starting mp node with REST API enabled...${NC}"
    
    # Change to the project root directory
    cd "$NODE_DIR" || {
        echo -e "${RED}ERROR: Could not change to project directory: $NODE_DIR${NC}"
        exit 1
    }
    
    # Start the node with REST API enabled in the background
    cargo build --bin mp-node
    sudo ./target/debug/mp-node --with-rest-api --log-level info > node_output.log 2>&1 &
    NODE_PID=$!
    
    echo "Node started with PID: $NODE_PID"
    echo "Node logs are being written to: $NODE_DIR/node_output.log"
    sleep 3
    # Wait for the node to initialize (check if REST API is available)
    echo "Waiting for node to initialize..."
    for i in {1..120}; do
        if curl -i "$REST_API_URL/health" > /dev/null 2>&1; then
            echo -e "${GREEN}Node is up and running!${NC}"
            return 0
        fi
        echo -n "."
        sleep 1
    done
    
    echo -e "\n${RED}ERROR: Node failed to start within 120 seconds${NC}"
    kill $NODE_PID
    return 1
}

# Function to generate an API key
generate_api_key() {
    echo -e "${YELLOW}Generating new API key...${NC}"
    
    API_KEY_RESPONSE=$(curl -i -X POST "${ADMIN_API_URL}/api-keys" \
        -H "Content-Type: application/json" \
        -d '{"address":"test_address"}')
    
    # Extract API key
    API_KEY=$(echo $API_KEY_RESPONSE | grep -o '"api_key":"[^"]*' | cut -d'"' -f4)
    
    if [ -n "$API_KEY" ]; then
        echo -e "${GREEN}API key generated successfully: ${API_KEY}${NC}"
        return 0
    else
        echo -e "${RED}Failed to generate API key${NC}"
        return 1
    fi
}

# Function to test the openai_proxy contract
test_openai_proxy_contract() {
    local api_key=$1
    
    echo -e "\n${YELLOW}Testing openai_proxy smart contract...${NC}"
    create_container_response=$(curl -i -s -X POST "${REST_API_URL}/cvm/create_container" \
        -H "Content-Type: application/json" \
        -H "X-API-Key: ${api_key}" \
        -d '{"agent_name": "openai_proxy", "description": "openai_proxy service", "name": "openai_proxy", "authorization_type": "APIKEY", "path": "test", "daily_call_quote": 100,"docker_compose": "version: \"3\"\nservices:\n  openai_proxy:\n    image: tenetwork/openai_proxy:latest\n    ports:\n    - 8100:8100\n    restart: always\n    environment: {}\n"}')
    
    echo "Response: ${create_container_response}"

    # List all containers
    echo -e "\n${YELLOW}Listing all containers...${NC}"
    LIST_CONTAINERS_RESPONSE=$(curl -i -s -X GET "${REST_API_URL}/cvm/list_containers" \
        -H "Content-Type: application/json" \
        -H "X-API-Key: ${api_key}" \
        -d '{}')
    
    echo "Response: ${LIST_CONTAINERS_RESPONSE}"
    CONTRACT_ADDR=0x34caa9a5f3c849f58f401fccbd58c3bd

    # Call the openai_proxy contract
    CALL_RESPONSE=$(curl -i -s -X POST "${REST_API_URL}/${CONTRACT_ADDR}/v1/chat/completions" \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer ${OPENAI_API_KEY}" \
        -d '{"model": "gpt-3.5-turbo", "messages": [{"role": "user", "content": "Hello, how are you?"}]}')
    
    echo "Response: ${CALL_RESPONSE}"
}

# Function to clean up resources
cleanup() {
    echo -e "\n${YELLOW}Cleaning up resources...${NC}"
    
    if [ -n "$NODE_PID" ]; then
        echo "Stopping mp node (PID: $NODE_PID)..."
        kill $NODE_PID
        wait $NODE_PID 2>/dev/null
        echo -e "${GREEN}Node stopped${NC}"
    fi
    
    if [ -n "$WEB2_STYLE_PID" ]; then
        echo "Stopping openai_proxy example (PID: $WEB2_STYLE_PID)..."
        kill $WEB2_STYLE_PID
        wait $WEB2_STYLE_PID 2>/dev/null
        echo -e "${GREEN}openai_proxy example stopped${NC}"
    fi
}

# Set up trap to ensure cleanup on script exit
trap cleanup EXIT INT TERM

# Main execution flow
main() {
    if [ -z "$OPENAI_API_KEY" ]; then
    echo "❌ 环境变量 OPENAI_API_KEY 未设置"
    echo "You can set it by running: export OPENAI_API_KEY=your_api_key"
    exit 1
    else
    echo "✅ OPENAI_API_KEY 已设置"
    fi

    # Check if ports are available
    check_port_usage || exit 1
    
    # Start mp node
    start_mp_node || exit 1
    
    # Generate API key
    generate_api_key || exit 1
    
    # Test openai_proxy contract
    test_openai_proxy_contract "$API_KEY"
    
    echo -e "\n${GREEN}All tests completed successfully!${NC}"
}

# Execute main function
main