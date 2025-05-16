//! tests/integration_tests.rs
//!
//! Runs the three Lua-rule paths (TG_FLOOD, TG_REPEAT, TG_SUSPICIOUS) against a
//! **real** Rspamd + Redis back-end.  Rspamd is accessed through the
//! crate-exported `scan_msg` helper, Redis through the `redis` crate.
//
//! Redis **must** be running on 127.0.0.1:6379
//! Rspamd **must** be running on 127.0.0.1:11333 with the bot’s Lua rules.

use chrono::Utc;
use redis::Commands;
use rspamd_telegram_bot::handlers::scan_msg;
use teloxide::types::{ChatId, Message, MessageId, UserId};
use serde_json::json;

// ────────────────────────────────────────────────────────────────────────────
// Helpers
// ────────────────────────────────────────────────────────────────────────────

/// Flush Redis before each test
fn flush_redis() {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut conn = client.get_connection().unwrap();
    let _: () = conn.flushdb().unwrap();
}

/// Quickly build a `teloxide::types::Message` via JSON (avoids filling every
/// optional field by hand).
fn make_message(chat: i64, user: i64, text: &str, msg_id: i32) -> Message {
    let raw = json!({
        "message_id": msg_id,
        "date": Utc::now().timestamp(),
        "chat": { "id": chat, "type": "private", "title": "Test" },
        "from": { "id": user, "is_bot": false, "first_name": "Testing", "username": "Tester" },
        "text": text
    });
    serde_json::from_value(raw).unwrap()
}

// ────────────────────────────────────────────────────────────────────────────
// 1. TG_FLOOD  –  31st message must contain the symbol ­& stats++
// ────────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn tg_flood_sets_symbol_and_increments_stats() {
    flush_redis();

    let chat_id = 1001;
    let user_id = 42;

    // 30 benign messages
    for i in 1..=30 {
        let _ = scan_msg(
            make_message(chat_id, user_id, &format!("msg{i}"), i),
            format!("msg{i}"),
        )
        .await
        .unwrap();
    }

    // 31-st message – should trigger TG_FLOOD
    let reply = scan_msg(
        make_message(chat_id, user_id, "the flood!", 31),
        "the flood!".into(),
    )
    .await
    .unwrap();

    assert!(
        reply.symbols.contains_key("TG_FLOOD"),
        "Expected TG_FLOOD after 31 rapid messages"
    );

    // verify stats in Redis
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut conn = client.get_connection().unwrap();
    let deleted: i64 = conn.hget("tg:stats", "deleted").unwrap_or(0);
    assert_eq!(deleted, 1, "deleted counter must be 1");
}

// ────────────────────────────────────────────────────────────────────────────
// 2. TG_REPEAT – 6 identical messages ⇒ symbol & rep += 1
// ────────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn tg_repeat_sets_symbol_and_increments_rep() {
    flush_redis();

    let chat_id = 2002;
    let user_id = 99;

    for i in 1..=6 {
        let _ = scan_msg(
            make_message(chat_id, user_id, "RepeatMe", i),
            "RepeatMe".into(),
        )
        .await
        .unwrap();
    }

    // Last call returns the sixth scan result
    let reply = scan_msg(
        make_message(chat_id, user_id, "RepeatMe", 7),
        "RepeatMe".into(),
    )
    .await
    .unwrap();

    assert!(
        reply.symbols.contains_key("TG_REPEAT"),
        "Expected TG_REPEAT symbol"
    );

    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut conn = client.get_connection().unwrap();
    let rep: i64 = conn.hget("tg:users:99", "rep").unwrap_or(0);
    assert_eq!(rep, 1, "user reputation must be 1 after one repeat offence");
}

// ────────────────────────────────────────────────────────────────────────────
// 3. TG_SUSPICIOUS – rep > 10 ⇒ symbol & deletion
// ────────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn tg_suspicious_sets_symbol() {
    flush_redis();

    let chat_id = 3003;
    let user_id = 123;

    // Manually bump reputation above the threshold
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut conn = client.get_connection().unwrap();
    let _: () = conn.hset("tg:users:123", "rep", 11).unwrap();

    let reply = scan_msg(
        make_message(chat_id, user_id, "Hello", 1),
        "Hello".into(),
    )
    .await
    .unwrap();

    assert!(
        reply.symbols.contains_key("TG_SUSPICIOUS"),
        "Expected TG_SUSPICIOUS for high-rep user"
    );
}

// ────────────────────────────────────────────────────────────────────────────
// 4. /make_admin      – pending, needs public handler
// 5. /reputation cmd  – pending, needs public handler
// ────────────────────────────────────────────────────────────────────────────
#[ignore]
#[tokio::test]
async fn make_admin_flow() {
    unimplemented!("exposed handler for /make_admin not yet available");
}

#[ignore]
#[tokio::test]
async fn reputation_command_flow() {
    unimplemented!("exposed handler for /reputation not yet available");
}
