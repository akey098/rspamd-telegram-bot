#!/bin/bash

# Rspamd Telegram Bot Deployment Script
# This script deploys the bot with fuzzy storage support

set -e

echo "🚀 Deploying Rspamd Telegram Bot with Fuzzy Storage..."

# Check if .env file exists
if [ ! -f .env ]; then
    echo "❌ Error: .env file not found!"
    echo "Please copy env.example to .env and configure your Telegram bot token:"
    echo "cp env.example .env"
    echo "Then edit .env and add your TELOXIDE_TOKEN"
    exit 1
fi

# Load environment variables
source .env

# Check if TELOXIDE_TOKEN is set
if [ -z "$TELOXIDE_TOKEN" ] || [ "$TELOXIDE_TOKEN" = "your_telegram_bot_token_here" ]; then
    echo "❌ Error: TELOXIDE_TOKEN not configured in .env file!"
    echo "Please edit .env and add your Telegram bot token"
    exit 1
fi

# Create necessary directories
echo "📁 Creating directories..."
mkdir -p logs data

# Build and start services
echo "🔨 Building and starting services..."
docker-compose up -d --build

# Wait for services to be ready
echo "⏳ Waiting for services to start..."
sleep 10

# Check if services are running
echo "🔍 Checking service status..."
docker-compose ps

# Test Rspamd connection
echo "🧪 Testing Rspamd connection..."
if curl -s http://localhost:11334/stat > /dev/null; then
    echo "✅ Rspamd controller is accessible"
else
    echo "⚠️  Rspamd controller not yet ready, please wait a moment"
fi

echo ""
echo "🎉 Deployment completed!"
echo ""
echo "📊 Service URLs:"
echo "  - Rspamd Controller: http://localhost:11334"
echo "  - Rspamd Scan: http://localhost:11333"
echo "  - Fuzzy Worker: localhost:11335 (UDP)"
echo ""
echo "📝 Next steps:"
echo "  1. Check logs: docker-compose logs -f"
echo "  2. Test fuzzy storage: ./scripts/test_fuzzy.sh"
echo "  3. Monitor bot: docker-compose logs -f bot"
echo ""
echo "🛑 To stop: docker-compose down"
