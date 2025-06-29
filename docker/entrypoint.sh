#!/bin/bash

set -e

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

exec /usr/local/bin/rspamd-telegram-bot
echo "Telegram bot started"