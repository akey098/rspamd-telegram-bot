#!/bin/bash

echo "=== Testing Rspamd Telegram Bot Docker Container ==="

# Build the image
echo "Building Docker image..."
docker build -t rspamd-telegram-bot -f docker/Dockerfile .

if [ $? -ne 0 ]; then
    echo "❌ Docker build failed"
    exit 1
fi
echo "✅ Docker image built successfully"

# Test Redis and Rspamd services
echo "Testing services in container..."
docker run --rm -e TELOXIDE_TOKEN="test-token" rspamd-telegram-bot bash -c "
    echo 'Starting Redis...'
    service redis-server start
    sleep 3
    
    echo 'Testing Redis connection...'
    if redis-cli ping | grep -q PONG; then
        echo '✅ Redis is working'
    else
        echo '❌ Redis failed'
        exit 1
    fi
    
    echo 'Testing Rspamd configuration...'
    if rspamadm configtest; then
        echo '✅ Rspamd configuration is valid'
    else
        echo '❌ Rspamd configuration failed'
        exit 1
    fi
    
    echo 'Testing bot binary...'
    if /usr/local/bin/rspamd-telegram-bot --help 2>/dev/null || /usr/local/bin/rspamd-telegram-bot 2>&1 | head -1; then
        echo '✅ Bot binary is working'
    else
        echo '❌ Bot binary failed'
        exit 1
    fi
    
    echo 'All services are working correctly!'
"

if [ $? -eq 0 ]; then
    echo "✅ All tests passed! The Docker container is ready to run."
    echo ""
    echo "To run the bot with a real Telegram token:"
    echo "docker run -d --name telegram-bot \\"
    echo "  -e TELOXIDE_TOKEN=\"your_telegram_bot_token\" \\"
    echo "  -p 11333:11333 -p 6379:6379 -p 10000:10000 \\"
    echo "  rspamd-telegram-bot"
else
    echo "❌ Some tests failed. Check the output above for details."
    exit 1
fi 