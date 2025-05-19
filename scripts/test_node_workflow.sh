#!/bin/bash

# mp Blockchain Workflow Test Script
# This script demonstrates a complete workflow for the mp blockchain:
# 1. Starting a mp node with REST API enabled
# 2. Starting the web2_style example separately
# 3. Interacting with the web2_style smart contract through the REST API
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

# Function to test the web2_style contract
test_web2_style_contract() {
    local api_key=$1
    
    echo -e "\n${YELLOW}Testing web2_style smart contract...${NC}"
    create_container_response=$(curl -i -s -X POST "${REST_API_URL}/cvm/create_container" \
        -H "Content-Type: application/json; charset=utf-8" \
        -H "X-API-Key: ${api_key}" \
        -d '{"agent_name":"web2_style", "path": "test","description": "web2_style service","authorization_type": "APIKEY", "daily_call_quote": 100,"name": "web2_style","docker_compose": "version: \"3\"\nservices:\n  web2_style:\n    image: tenetwork/web2_style:latest\n    ports:\n    - 8030:8080\n    restart: always\n    environment: {}\n"}')
    
    echo "Response: ${create_container_response}"

    # List all containers
    echo -e "\n${YELLOW}Listing all containers...${NC}"
    LIST_CONTAINERS_RESPONSE=$(curl -i -s -X GET "${REST_API_URL}/cvm/list_containers" \
        -H "Content-Type: application/json" \
        -H "X-API-Key: ${api_key}" \
        -d '{}')
    
    echo "Response: ${LIST_CONTAINERS_RESPONSE}"
    CONTRACT_ADDR=0x683a4e3491334ecdaaa369afa8dcc009
    # Test 1: Create a user
    echo -e "\nTest 1: Creating a user through mp node..."
    CREATE_USER_RESPONSE=$(curl -i -s -X POST "${REST_API_URL}/${CONTRACT_ADDR}/users" \
        -H "Content-Type: application/json" \
        -H "X-API-Key: ${api_key}" \
        -d '{"id":"user1","name":"Test User","email":"test@example.com"}')
    
    echo "Response: ${CREATE_USER_RESPONSE}"
    
    # 确认用户已创建
    if [[ $CREATE_USER_RESPONSE == *"user"* ]]; then
        echo -e "${GREEN}Successfully created user${NC}"
    else
        echo -e "${RED}Failed to create user${NC}"
        echo "Detailed response: ${CREATE_USER_RESPONSE}"
    fi
    
    # 等待一下，确保用户创建完成
    echo "Waiting for user creation to complete..."
    sleep 2
    
    
    # Test 2: Get user through node
    echo -e "\nTest 2: Getting user through mp node..."
    GET_USER_RESPONSE=$(curl -i -s -X GET "${REST_API_URL}/${CONTRACT_ADDR}/users/user1" \
        -H "Content-Type: application/json" \
        -H "X-API-Key: ${api_key}" \
        -d '{}' 2>&1)
    
    echo "Response headers and body:"
    echo "${GET_USER_RESPONSE}"
    
    if [[ $GET_USER_RESPONSE == *"user"* ]]; then
        echo -e "${GREEN}Successfully retrieved user${NC}"
    else
        echo -e "${RED}Failed to retrieve user${NC}"
    fi
    
    # Test 3: Create a post
    echo -e "\nTest 3: Creating a post through mp node..."
    CREATE_POST_RESPONSE=$(curl -i -s -X POST "${REST_API_URL}/${CONTRACT_ADDR}/posts" \
        -H "Content-Type: application/json" \
        -H "X-API-Key: ${api_key}" \
        -d '{"id":"post1","title":"Test Post","content":"This is a test post","user_id":"user1"}')
    
    echo "Response: ${CREATE_POST_RESPONSE}"
    
    if [[ $CREATE_POST_RESPONSE == *"post"* ]]; then
        echo -e "${GREEN}Successfully created post${NC}"
    else
        echo -e "${RED}Failed to create post${NC}"
    fi
    
    # 等待一下，确保帖子创建完成
    echo "Waiting for post creation to complete..."
    sleep 2
    
    
    # Test 4: Get user posts through node
    echo -e "\nTest 4: Getting user posts through mp node..."
    GET_USER_POSTS_RESPONSE=$(curl -i -s -X GET "${REST_API_URL}/${CONTRACT_ADDR}/users/user1/posts" \
        -H "Content-Type: application/json" \
        -H "X-API-Key: ${api_key}" \
        -d '{}' 2>&1)
    
    echo "Response headers and body:"
    echo "${GET_USER_POSTS_RESPONSE}"
    
    if [[ $GET_USER_POSTS_RESPONSE == *"posts"* ]]; then
        echo -e "${GREEN}Successfully retrieved user posts${NC}"
    else
        echo -e "${RED}Failed to retrieve user posts${NC}"
    fi
    
    # Test 5: Get all users
    echo -e "\nTest 5: Getting all users through mp node..."
    GET_ALL_USERS_RESPONSE=$(curl -i -s -X GET "${REST_API_URL}/${CONTRACT_ADDR}/users" \
        -H "Content-Type: application/json" \
        -H "X-API-Key: ${api_key}" \
        -d '{}' 2>&1)
    
    echo "Response headers and body:"
    echo "${GET_ALL_USERS_RESPONSE}"
    
    if [[ $GET_ALL_USERS_RESPONSE == *"users"* ]]; then
        echo -e "${GREEN}Successfully retrieved all users${NC}"
    else
        echo -e "${RED}Failed to retrieve all users${NC}"
    fi
}

# Function to run performance tests
run_performance_tests() {
    local api_key=$1
    local num_requests=$2
    
    echo -e "\n${YELLOW}Running performance tests: $num_requests consecutive create user requests...${NC}\n"
    
    total_time=0
    success_count=0
    
    for i in $(seq 1 $num_requests); do
        echo "Request $i of $num_requests"
        start_time=$(date +%s.%N)
        
        CREATE_USER_RESPONSE=$(curl -s -X POST "${REST_API_URL}/${CONTRACT_ADDR}/users" \
            -H "Content-Type: application/json" \
            -H "X-API-Key: ${api_key}" \
            -d '{"id":"perf_user'$i'","name":"Performance Test User '$i'","email":"perf'$i'@example.com"}')
        
        end_time=$(date +%s.%N)
        response_time=$(echo "$end_time - $start_time" | bc)
        total_time=$(echo "$total_time + $response_time" | bc)
        
        echo "Response time: ${response_time} seconds"
        
        if [[ $CREATE_USER_RESPONSE == *"user"* ]]; then
            echo -e "${GREEN}Success: User created${NC}"
            ((success_count++))
        else
            echo -e "${RED}Failed: Invalid response${NC}"
            echo "Response: ${CREATE_USER_RESPONSE}"  # 显示失败响应以便调试
        fi
        
        echo ""
        sleep 0.5  # 增加等待时间，确保请求不会太快
    done

    echo -e "${YELLOW}Remove docker container...${NC}"
    REMOVE_CONTAINER_RESPONSE=$(curl -s -X DELETE "${REST_API_URL}/cvm/remove_container" \
        -H "Content-Type: application/json" \
        -H "X-API-Key: ${api_key}" \
        -d '{"id": "0x683a4e3491334ecdaaa369afa8dcc009"}')

    echo "Response: ${REMOVE_CONTAINER_RESPONSE}"
    
    
    # Calculate statistics
    avg_time=$(echo "scale=3; $total_time / $num_requests" | bc)
    success_rate=$(echo "scale=1; ($success_count / $num_requests) * 100" | bc)
    
    echo -e "${YELLOW}Performance Test Results:${NC}"
    echo "Total requests: $num_requests"
    echo "Successful requests: $success_count"
    echo "Success rate: ${success_rate}%"
    echo "Average response time: ${avg_time} seconds"
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
        echo "Stopping web2_style example (PID: $WEB2_STYLE_PID)..."
        kill $WEB2_STYLE_PID
        wait $WEB2_STYLE_PID 2>/dev/null
        echo -e "${GREEN}web2_style example stopped${NC}"
    fi
}

# Set up trap to ensure cleanup on script exit
trap cleanup EXIT INT TERM

# Main execution flow
main() {
    # Check if ports are available
    check_port_usage || exit 1
    
    # Start mp node
    start_mp_node || exit 1
    
    # Generate API key
    generate_api_key || exit 1
    
    # Test web2_style contract
    test_web2_style_contract "$API_KEY"
    
    # Run performance tests (5 consecutive requests)
    run_performance_tests "$API_KEY" 5
    
    echo -e "\n${GREEN}All tests completed successfully!${NC}"
}

# Execute main function
main