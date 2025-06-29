# ┌─────────────────────────────────────────────────────┐
# │ Stage 1: Build the Telegram bot binary             │
# └─────────────────────────────────────────────────────┘
FROM rust:1.86-slim AS builder

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
      pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app
COPY Cargo.toml Cargo.lock ./

# Cache dependencies
RUN mkdir src && \
    echo 'fn main() { println!("dummy"); }' > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy all sources and rebuild the actual bot
COPY src/ ./src/
COPY Cargo.toml ./
COPY Cargo.lock ./
COPY rspamd-config/ ./rspamd-config/
COPY docker/entrypoint.sh /entrypoint.sh
RUN cargo build --release
RUN ls -l /usr/src/app/target/release/
RUN apt-get update && apt-get install -y --no-install-recommends file && file /usr/src/app/target/release/rspamd-telegram-bot

# ┌─────────────────────────────────────────────────────┐
# │ Stage 2: Runtime with Debian + Rspamd + Redis      │
# └─────────────────────────────────────────────────────┘
FROM debian:bookworm-slim

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      psmisc lsb-release wget gpg ca-certificates sudo redis-server \
 && rm -rf /var/lib/apt/lists/*

# Add Rspamd repository
RUN CODENAME=$(lsb_release -c -s) && \
    mkdir -p /etc/apt/keyrings && \
    wget -O- https://rspamd.com/apt-stable/gpg.key | gpg --dearmor | tee /etc/apt/keyrings/rspamd.gpg > /dev/null && \
    echo "deb [signed-by=/etc/apt/keyrings/rspamd.gpg] http://rspamd.com/apt-stable/ $CODENAME main" | tee /etc/apt/sources.list.d/rspamd.list && \
    echo "deb-src [signed-by=/etc/apt/keyrings/rspamd.gpg] http://rspamd.com/apt-stable/ $CODENAME main" | tee -a /etc/apt/sources.list.d/rspamd.list

RUN apt-get update && \
    apt-get install -y --no-install-recommends rspamd && \
    rm -rf /var/lib/apt/lists/*

# Copy all Lua files and settings.conf
COPY rspamd-config/lua.local.d /etc/rspamd/lua.local.d/
RUN mkdir -p /etc/rspamd/lua && \
    cp -R /etc/rspamd/lua.local.d/telegram /etc/rspamd/lua/ && \
    rm -f /etc/rspamd/lua.local.d/telegram.lua /etc/rspamd/lua.local.d/whiteblacklist.lua

COPY rspamd-config/modules.local.d /etc/rspamd/modules.local.d/

# Verify Rspamd configuration at build time
RUN rspamadm configtest

# Copy the built bot
COPY --from=builder /usr/src/app/target/release/rspamd-telegram-bot /usr/local/bin/rspamd-telegram-bot

# Copy entrypoint script
COPY docker/entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

EXPOSE 11333 6379

ENTRYPOINT ["sh","-c", "\
    echo \"Cleaning ports…\"; \
    fuser -k 11333/tcp 2>/dev/null || true; \
    fuser -k 6379/tcp 2>/dev/null || true; \
    exec /entrypoint.sh\
"]

