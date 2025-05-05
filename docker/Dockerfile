# Stage 1: Build the Telegram bot binary
FROM rust:latest AS builder
WORKDIR /usr/src/app
COPY Cargo.toml Cargo.lock ./
COPY ../Cargo.toml ../Cargo.lock ./
RUN mkdir src && echo 'fn main() { }' > src/main.rs
RUN cargo build --release

RUN rm src/main.rs
COPY ../src ./src
RUN cargo build --release


# Stage 2: Runtime image with Rspamd and Redis
FROM rspamd/rspamd:latest
USER root
# Install redis server
RUN apt-get update && \
    apt-get install -y redis-server && \
    rm -rf /var/lib/apt/lists/*

# Copy the compiled bot and Lua rules into the image
COPY --from=builder /usr/src/app/target/release/rspamd-telegram-bot /usr/local/bin/rspamd-telegram-bot
COPY rspamd-config/lua.local.d/telegram.lua /etc/rspamd/lua.local.d/telegram.lua

# Expose ports if needed (11333 for Rspamd, 6379 for Redis, etc.)
EXPOSE 11333 6379

# Start all services (Redis, Rspamd, and the bot)
CMD service redis-server start \
  && service rspamd start \
  && exec /usr/local/bin/rspamd-telegram-bot