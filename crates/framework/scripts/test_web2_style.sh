#!/bin/bash

set -e  # Exit immediately if a command exits with a non-zero status

# Define colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# # Clean up previous user and post data (if needed)
# echo "----------------------------------------"
# echo "Cleaning up previous data..."
# curl -X DELETE "http://127.0.0.1:8080/users?id=user1" || echo -e "${RED}Failed to delete user.${NC}"
# curl -X DELETE "http://127.0.0.1:8080/posts?id=post1" || echo -e "${RED}Failed to delete post.${NC}"
# echo "----------------------------------------"

# Create a user
echo "Creating user..."
response=$(curl -s -o response.json -w "%{http_code}" -X POST -H "Content-Type: application/json" -d '{"id": "user1", "name": "John Doe", "email": "johndoe@example.com"}' http://127.0.0.1:8080/users)
echo "Status code: $response"
if [ "$response" -eq 201 ]; then
    echo -e "${GREEN}User created successfully.${NC}"
    echo "Response content:"
    cat response.json | jq .  # Output the response content for the created user
else
    echo -e "${RED}Failed to create user. Status code: $response${NC}"
    cat response.json
    exit 1
fi
echo "----------------------------------------"

# Create a post
echo "Creating post..."
response=$(curl -s -o response.json -w "%{http_code}" -X POST -H "Content-Type: application/json" -d '{"id": "post1", "user_id": "user1", "title": "My First Post", "content": "This is the content of my first post. Hello everyone!"}' http://127.0.0.1:8080/posts)
echo "Status code: $response"
if [ "$response" -eq 201 ]; then
    echo -e "${GREEN}Post created successfully.${NC}"
    echo "Response content:"
    cat response.json | jq .  # Output the response content for the created post
else
    echo -e "${RED}Failed to create post. Status code: $response${NC}"
    cat response.json
    exit 1
fi
echo "----------------------------------------"

# Fetch user information
echo "Fetching user information..."
user_response=$(curl -s -X GET "http://127.0.0.1:8080/users/user1")
echo "User Information:"
echo "$user_response" | jq .
echo "----------------------------------------"

# Fetch post information
echo "Fetching post information..."
post_response=$(curl -s -X GET "http://127.0.0.1:8080/posts/post1")
echo "Post Information:"
echo "$post_response" | jq .
echo "----------------------------------------"

# Clean up temporary files
rm -f response.json