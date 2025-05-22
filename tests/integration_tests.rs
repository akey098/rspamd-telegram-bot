//! tests/integration_tests.rs
//!
//! Runs the three Lua-rule paths (TG_FLOOD, TG_REPEAT, TG_SUSPICIOUS) against a
//! **real** Rspamd + Redis back-end.  Rspamd is accessed through the
//! crate-exported `scan_msg` helper, Redis through the `redis` crate.
//
//! Redis **must** be running on 127.0.0.1:6379
//! Rspamd **must** be running on 127.0.0.1:11333 with the bot’s Lua rules.

use std::time::Duration;

use chrono::Utc;
use redis::Commands;
use rspamd_telegram_bot::handlers::scan_msg;
use teloxide::types::{Chat, ChatId, ChatKind, ChatPrivate, MediaKind, MediaText, Message, MessageCommon, MessageId, MessageKind, User, UserId};

/// Flush Redis before each test
fn flush_redis() {
    let client = redis::Client::open("redis://127.0.0.1/")
        .expect("Failed to connect to Redis");
    let mut conn = client
        .get_connection()
        .expect("Failed to connect to Redis");
    let _: () = conn
        .flushdb()
        .expect("Failed to flush Redis");
}

fn make_user(id: u64, username: &str) -> User {
    User {
        id: UserId(id),
        is_bot: false,
        first_name: username.into(),
        last_name: None,
        username: Some(username.into()),
        language_code: None,
        is_premium: false,
        added_to_attachment_menu: false,
    }
}

// manually build a private chat
fn make_chat(chat_id: i64) -> Chat {
    let private_chat: ChatPrivate = ChatPrivate {
        username: Some("Anon".into()),
        first_name: Some("Anon".into()),
        last_name: None,
    };
    Chat {
        id: ChatId(chat_id),
        kind: ChatKind::Private(private_chat)
    }
}

// manually build a Message with text
fn make_message(chat_id: i64, user_id: u64, username: &str, text: &str, msg_id: i32) -> Message {
    let user = make_user(user_id, username);
    let chat = make_chat(chat_id);
    Message {
        id: MessageId(msg_id),
        date: Utc::now(),
        chat,
        kind: MessageKind::Common(MessageCommon {
            author_signature: None,
            effect_id: None,
            forward_origin: None,
            reply_to_message: None,
            external_reply: None,
            quote: None,
            reply_to_story: None,
            sender_boost_count: None,
            edit_date: None,
            media_kind: MediaKind::Text(MediaText {
                text: text.into(),
                entities: Vec::new(),
                link_preview_options: None
            }),
            reply_markup: None,
            is_automatic_forward: false,
            has_protected_content: false,
            is_from_offline: false,
            business_connection_id: None
        }),
        thread_id: None,
        from: Some(user),
        sender_chat: None,
        is_topic_message: false,
        via_bot: None,
        sender_business_bot: None
    }
}

// ────────────────────────────────────────────────────────────────────────────
// 1. TG_FLOOD  –  31st message must contain the symbol ­& stats++
// ────────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn tg_flood_sets_symbol_and_increments_stats() {
    flush_redis();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let chat_id = 1001;
    let user_id = 42;
    let key = format!("tg:users:{}", user_id);
    
    let client = redis::Client::open("redis://127.0.0.1/")
        .expect("Failed to connect to Redis");
    let mut conn = client
        .get_connection()
        .expect("Failed to connect to Redis");
    let _: () = conn
        .hset(key.clone(), "rep", 0)
        .expect("Failed to set user reputation");

    // 30 benign messages
    for i in 1..=30 {
        scan_msg(
            make_message(chat_id, user_id, "test", &format!("msg{i}"), i),
            format!("msg{i}"),
        )
            .await
            .ok()
            .unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // 31-st message – should trigger TG_FLOOD
    let reply = scan_msg(
        make_message(chat_id, user_id, "test", "the flood!", 31),
        "the flood!".into(),
    )
    .await
    .ok()
    .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    assert!(
        reply.symbols.contains_key("TG_FLOOD"),
        "Expected TG_FLOOD after 31 rapid messages"
    );
    
    let rep: i64 = conn
        .hget(key.clone(), "rep")
        .expect("Failed to get rep");
    assert_eq!(rep, 1);
}

// ────────────────────────────────────────────────────────────────────────────
// 2. TG_REPEAT – 6 identical messages ⇒ symbol & rep += 1
// ────────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn tg_repeat_sets_symbol_and_increments_rep() {
    flush_redis();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let chat_id = 2002;
    let user_id = 99;
    let key = format!("tg:users:{}", user_id);

    let client = redis::Client::open("redis://127.0.0.1/")
        .expect("Failed to connect to Redis");
    let mut conn = client
        .get_connection()
        .expect("Failed to connect to Redis");
    let _: () = conn
        .hset(key.clone(), "rep", 0)
        .expect("Failed to set user reputation");

    for i in 1..=6 {
        let _ = scan_msg(
            make_message(chat_id, user_id,"test", "RepeatMe", i),
            "RepeatMe".into(),
        )
        .await
        .ok()
        .unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // Last call returns the sixth scan result
    let reply = scan_msg(
        make_message(chat_id, user_id,"test", "RepeatMe", 7),
        "RepeatMe".into(),
    )
    .await
    .ok()
    .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    assert!(
        reply.symbols.contains_key("TG_REPEAT"),
        "Expected TG_REPEAT symbol"
    );
    let rep: i64 = conn
        .hget(key.clone(), "rep")
        .expect("Failed to get rep");
    assert_eq!(rep, 1);
}

// ────────────────────────────────────────────────────────────────────────────
// 3. TG_SUSPICIOUS – rep > 10 ⇒ symbol & deletion
// ────────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn tg_suspicious_sets_symbol() {
    flush_redis();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let chat_id = 3003;
    let user_id = 123;
    let key = format!("tg:users:{}", user_id);

    // Manually bump reputation above the threshold
    let client = redis::Client::open("redis://127.0.0.1/")
        .expect("Failed to connect to Redis");
    let mut conn = client
        .get_connection()
        .expect("Failed to connect to Redis");
    let _: () = conn
        .hset(key.clone(), "rep", 11)
        .expect("Failed to set user reputation");

    let reply = scan_msg(
        make_message(chat_id, user_id,"test", "Hello", 1),
        "Hello".into(),
    )
    .await
    .ok()
    .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    let rep: i64 = conn
        .hget(key.clone(), "rep")
        .expect("Failed to get user reputation");
    assert!(
        reply.symbols.contains_key("TG_SUSPICIOUS"),
        "Expected TG_SUSPICIOUS for high-rep user"
    );
    assert_eq!(rep, 12);
}

// ────────────────────────────────────────────────────────────────────────────
// 4. /make_admin      – pending, needs public handler
// 5. /reputation cmd  – pending, needs public handler
// ────────────────────────────────────────────────────────────────────────────
#[ignore]
#[tokio::test]
async fn make_admin_integration_flow() {
    unimplemented!("exposed handler for /makeadmin not yet available");
}


#[ignore]
#[tokio::test]
async fn reputation_command_flow() {
    unimplemented!("exposed handler for /reputation not yet available");
}
