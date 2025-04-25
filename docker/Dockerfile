# ┌───────────────────────────────────────────────────────────────┐
# │ docker/Dockerfile                                           │
# └───────────────────────────────────────────────────────────────┘

# 1) Builder stage: compile the Rust bot
FROM rust:1.86-slim AS builder

# Install system deps for crates like openssl-sys
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
      pkg-config \
      libssl-dev && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/rspamd-telegram-bot

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && \
    echo 'fn main() { println!("dummy"); }' > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy actual source & build
COPY . .
RUN cargo build --release

# 2) Runtime stage: slim image with Rspamd & bot
FROM debian:bookworm-slim

# Install Rspamd, CA certs, and runtime libs
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
      rspamd \
      ca-certificates \
      libssl1.1 && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/rspamd-telegram-bot/target/release

# Copy the bot binary
COPY --from=builder /usr/src/rspamd-telegram-bot/target/release/rspamd-telegram-bot .

# Copy your rspamd-config directory so deploy_settings() can find it
COPY --from=builder /usr/src/rspamd-telegram-bot/rspamd-config ../rspamd-config

# The bot will read BOT_TOKEN from env—no .env needed
CMD ["./rspamd-telegram-bot"]
