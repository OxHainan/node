#!/bin/bash

# Script to start mp node with REST API enabled
# This script starts the mp node with the optimized REST API

# Set default values
REST_API_ENABLED="--with-rest-api"
LOG_LEVEL="info"

# Print banner
echo "==============================================="
echo "  Starting mp Node with Optimized REST API"
echo "==============================================="
echo "REST API Request Port: 3000"
echo "REST API Admin Port: 3001"
echo "Log Level: $LOG_LEVEL"
echo "Data Directory: $DATA_DIR"
echo "==============================================="

# Check if there's already a node running on ports 3000 or 3001
if lsof -i :3000,3001 > /dev/null 2>&1; then
    echo "ERROR: Ports 3000 or 3001 are already in use."
    echo "Please stop any running mp node instances before starting a new one."
    echo "You can use: lsof -i :3000,3001 to see which processes are using these ports."
    echo "Then use: kill <PID> to terminate the process."
    exit 1
fi

# Start the node with REST API enabled
echo "Starting mp node..."
cd "$(dirname "$0")/.." || exit 1
cargo run --bin mp-node -- $REST_API_ENABLED --log-level $LOG_LEVEL

# Note: This script will block until the node is stopped
