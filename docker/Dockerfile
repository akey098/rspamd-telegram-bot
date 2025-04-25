# docker/Dockerfile

# 1) Builder: compile your Rust bot
FROM rust:1.86-slim AS builder

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
      pkg-config \
      libssl-dev && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/rspamd-telegram-bot
COPY Cargo.toml Cargo.lock ./

# Pre–fetch deps
RUN mkdir src && \
    echo 'fn main() { println!("dummy"); }' > src/main.rs && \
    cargo build --release && \
    rm -rf src

COPY . .
RUN cargo build --release

# 2) Runtime: Debian + Rspamd + your bot
FROM debian:bookworm-slim

# a) Install tools for adding the Rspamd repo
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
      lsb-release \
      wget \
      gnupg \
      ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# b) Add Rspamd apt-stable repository using a keyring
RUN mkdir -p /etc/apt/keyrings && \
    wget -O- https://rspamd.com/apt-stable/gpg.key \
      | gpg --dearmor > /etc/apt/keyrings/rspamd.gpg && \
    echo "deb [signed-by=/etc/apt/keyrings/rspamd.gpg] \
      http://rspamd.com/apt-stable/ $(lsb_release -c -s) main" \
      > /etc/apt/sources.list.d/rspamd.list && \
    echo "deb-src [signed-by=/etc/apt/keyrings/rspamd.gpg] \
      http://rspamd.com/apt-stable/ $(lsb_release -c -s) main" \
      >> /etc/apt/sources.list.d/rspamd.list

# c) Install Rspamd (will pull in its libssl) and clean up
RUN apt-get update && \
    apt-get install -y --no-install-recommends rspamd && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/rspamd-telegram-bot/target/release

# Copy the bot binary and the rspamd-config directory
COPY --from=builder /usr/src/rspamd-telegram-bot/target/release/rspamd-telegram-bot .
COPY --from=builder /usr/src/rspamd-telegram-bot/rspamd-config ../rspamd-config

# Entrypoint: reads BOT_TOKEN from the environment
CMD ["./rspamd-telegram-bot"]
