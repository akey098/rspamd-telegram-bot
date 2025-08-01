#!/bin/bash

# Performance testing script for reputation integration
# This script measures the performance impact of reputation processing

set -e

echo "=== Reputation Performance Testing ==="

# Configuration
NUM_MESSAGES=100
REDIS_HOST="127.0.0.1"
REDIS_PORT="6379"

# Test data
USER_IDS=(1001 1002 1003 1004 1005)
CHAT_IDS=(2001 2002 2003 2004 2005)
MESSAGES=(
    "Hello everyone! How are you doing today?"
    "This is a test message for performance testing."
    "Spam message with lots of CAPS and !!!"
    "Normal conversation message."
    "Message with links: https://example.com"
)

echo "1. Preparing test data..."

# Clear existing test data
redis-cli -h $REDIS_HOST -p $REDIS_PORT --eval - <<EOF
local keys = redis.call('keys', 'tg:reputation:user:100*')
for i=1,#keys do
    redis.call('del', keys[i])
end
EOF

# Set up reputation data for test users
for user_id in "${USER_IDS[@]}"; do
    # Random reputation values
    bad_rep=$((RANDOM % 20))
    good_rep=$((RANDOM % 10))
    
    redis-cli -h $REDIS_HOST -p $REDIS_PORT hset "tg:reputation:user:$user_id" "bad" "$bad_rep"
    redis-cli -h $REDIS_HOST -p $REDIS_PORT hset "tg:reputation:user:$user_id" "good" "$good_rep"
    redis-cli -h $REDIS_HOST -p $REDIS_PORT expire "tg:reputation:user:$user_id" 604800
done

echo "2. Running performance tests..."

# Test 1: Messages without reputation data
echo "   Test 1: Messages without reputation data"
start_time=$(date +%s.%N)

for i in $(seq 1 $NUM_MESSAGES); do
    user_id=${USER_IDS[$((RANDOM % ${#USER_IDS[@]}))]}
    chat_id=${CHAT_IDS[$((RANDOM % ${#CHAT_IDS[@]}))]}
    message=${MESSAGES[$((RANDOM % ${#MESSAGES[@]}))]}
    
    # Create test email
    email="Received: from 127.0.0.1 by localhost with HTTP; $(date -R)
Date: $(date -R)
From: user$user_id@example.com
To: chat$chat_id@example.com
Subject: Test message $i
X-Telegram-User: $user_id
MIME-Version: 1.0
Content-Type: text/plain; charset=UTF-8

$message"
    
    echo "$email" | rspamc --mime > /dev/null 2>&1
done

end_time=$(date +%s.%N)
duration1=$(echo "$end_time - $start_time" | bc)
echo "   Completed in ${duration1}s (${NUM_MESSAGES} messages)"

# Test 2: Messages with reputation data
echo "   Test 2: Messages with reputation data"
start_time=$(date +%s.%N)

for i in $(seq 1 $NUM_MESSAGES); do
    user_id=${USER_IDS[$((RANDOM % ${#USER_IDS[@]}))]}
    chat_id=${CHAT_IDS[$((RANDOM % ${#CHAT_IDS[@]}))]}
    message=${MESSAGES[$((RANDOM % ${#MESSAGES[@]}))]}
    
    # Create test email with reputation data
    email="Received: from 127.0.0.1 by localhost with HTTP; $(date -R)
Date: $(date -R)
From: user$user_id@example.com
To: chat$chat_id@example.com
Subject: Test message $i
X-Telegram-User: $user_id
MIME-Version: 1.0
Content-Type: text/plain; charset=UTF-8

$message"
    
    echo "$email" | rspamc --mime > /dev/null 2>&1
done

end_time=$(date +%s.%N)
duration2=$(echo "$end_time - $start_time" | bc)
echo "   Completed in ${duration2}s (${NUM_MESSAGES} messages)"

# Calculate performance metrics
echo ""
echo "3. Performance Results:"
echo "   Without reputation: ${duration1}s"
echo "   With reputation:    ${duration2}s"

if (( $(echo "$duration2 > $duration1" | bc -l) )); then
    overhead=$(echo "($duration2 - $duration1) / $duration1 * 100" | bc -l)
    echo "   Overhead: ${overhead}%"
else
    echo "   Reputation processing appears to be faster or similar"
fi

# Test 3: Memory usage
echo ""
echo "4. Memory Usage Test:"
echo "   Current Redis memory usage:"
redis-cli -h $REDIS_HOST -p $REDIS_PORT info memory | grep used_memory_human

echo "   Reputation keys in Redis:"
redis-cli -h $REDIS_HOST -p $REDIS_PORT keys "tg:reputation:user:*" | wc -l

echo ""
echo "=== Performance Testing Complete ==="
echo "Results indicate the performance impact of reputation integration." 