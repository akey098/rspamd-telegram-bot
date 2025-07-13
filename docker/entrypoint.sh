#!/bin/bash

# Remove set -e to see errors
# set -e

service redis-server start


# Wait until Redis is available
for i in {1..20}; do
    if redis-cli ping | grep -q PONG; then
        echo "Redis is up!"
        break
    fi
    echo "Waiting for Redis... ($i)"
    sleep 1
done

if ! redis-cli ping | grep -q PONG; then
    echo "Redis did not start in time!" >&2
    exit 1
fi

ls /etc/rspamd/lua.local.d/

service rspamd restart

rspamadm configtest

echo "About to start the bot..."
echo "Bot binary exists: $(ls -la /usr/local/bin/rspamd-telegram-bot)"
echo "Current environment:"
env | grep -E "(TELOXIDE|RUST)" || echo "No TELOXIDE or RUST env vars found"

exec /usr/local/bin/rspamd-telegram-bot
echo "Telegram bot started"