#!/bin/bash

# Validation script for Rspamd reputation plugin configuration
# This script checks if the reputation plugin is properly configured and working

set -e

echo "=== Rspamd Reputation Plugin Validation ==="

# Check if Rspamd is running
echo "1. Checking Rspamd service status..."
if systemctl is-active --quiet rspamd; then
    echo "   ✓ Rspamd is running"
else
    echo "   ✗ Rspamd is not running"
    exit 1
fi

# Check reputation plugin configuration
echo "2. Checking reputation plugin configuration..."
if rspamadm configdump -c /etc/rspamd/rspamd.conf | grep -A 10 "reputation" | grep -q "enabled = true"; then
    echo "   ✓ Reputation plugin is enabled"
else
    echo "   ✗ Reputation plugin is not enabled or not found"
fi

# Check Redis connectivity
echo "3. Checking Redis connectivity..."
if redis-cli ping | grep -q "PONG"; then
    echo "   ✓ Redis is accessible"
else
    echo "   ✗ Redis is not accessible"
    exit 1
fi

# Check reputation configuration file
echo "4. Checking reputation configuration file..."
if [ -f "/etc/rspamd/local.d/reputation.conf" ]; then
    echo "   ✓ Reputation configuration file exists"
    echo "   Configuration content:"
    cat /etc/rspamd/local.d/reputation.conf | sed 's/^/   /'
else
    echo "   ✗ Reputation configuration file not found"
fi

# Test reputation plugin with a sample message
echo "5. Testing reputation plugin with sample message..."
SAMPLE_EMAIL="Received: from 127.0.0.1 by localhost with HTTP; $(date -R)
Date: $(date -R)
From: test@example.com
To: test@example.com
Subject: Test message
X-Telegram-User: 12345
MIME-Version: 1.0
Content-Type: text/plain; charset=UTF-8

This is a test message for reputation validation."

echo "$SAMPLE_EMAIL" | rspamc --mime > /tmp/rspamd_test_output 2>&1

if grep -q "USER_REPUTATION" /tmp/rspamd_test_output; then
    echo "   ✓ Reputation plugin is processing messages correctly"
else
    echo "   ⚠ Reputation plugin may not be configured correctly"
    echo "   Test output:"
    cat /tmp/rspamd_test_output | sed 's/^/   /'
fi

# Check Redis reputation data structure
echo "6. Checking Redis reputation data structure..."
if redis-cli keys "tg:reputation:user:*" | head -5 | wc -l | grep -q -v "^0$"; then
    echo "   ✓ Reputation data exists in Redis"
    echo "   Sample reputation keys:"
    redis-cli keys "tg:reputation:user:*" | head -3 | sed 's/^/   /'
else
    echo "   ⚠ No reputation data found in Redis (this is normal for fresh installation)"
fi

# Validate configuration syntax
echo "7. Validating Rspamd configuration syntax..."
if rspamadm configtest -c /etc/rspamd/rspamd.conf; then
    echo "   ✓ Rspamd configuration syntax is valid"
else
    echo "   ✗ Rspamd configuration has syntax errors"
    exit 1
fi

echo ""
echo "=== Validation Complete ==="
echo "If all checks passed, the reputation plugin should be working correctly."
echo "To test with actual data, run the integration tests:"
echo "  cargo test --test integration_tests" 