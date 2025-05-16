use redis::AsyncCommands;
use teloxide::prelude::*;
use teloxide_tests::MockBot;
use rspamd_telegram_bot::{run_dispatcher, scan_msg};

/// Helper to reset Redis state before each test
async fn reset_redis() {
    println!("[DEBUG] reset_redis: Connecting to Redis and flushing DB");
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut conn = client.get_async_connection().await.unwrap();
    let _ : () = conn.flushdb().await.unwrap();
    println!("[DEBUG] reset_redis: Redis flushed");
}

/// Simulate message sending to the bot via MockBot
async fn send_message(bot: &MockBot, chat_id: ChatId, user: User, text: &str) {
    println!("[DEBUG] send_message: chat_id={}, user_id={}, text=\"{}\"", chat_id.0, user.id, text);
    let msg = Update::new_message(
        Message::builder()
            .chat_id(chat_id.0)
            .message_id(1)
            .date(chrono::Utc::now())
            .text(text)
            .from(user.clone())
            .build()
    );
    bot.send_update(msg).await;
    println!("[DEBUG] send_message: update sent");
}

#[tokio::test]
async fn test_tg_flood_deletion() {
    println!("[DEBUG] Running test: test_tg_flood_deletion");
    reset_redis().await;
    let bot = MockBot::new();
    Dispatcher::builder(bot.clone(), run_dispatcher())
        .build()
        .dispatch();

    let chat = ChatId(1001);
    let user = User { id: UserId(42), is_bot: false, first_name: "Test".into(), ..Default::default() };

    println!("[DEBUG] Sending 31 messages to trigger flood detection");
    // Send 31 quick messages to trigger flood
    for i in 0..31 {
        send_message(&bot, chat, user.clone(), &format!("Spam {}", i)).await;
    }
    println!("[DEBUG] All messages sent, taking snapshot of requests");

    // After 31st, expect a DeleteMessage request
    let reqs = bot.requests_snapshot().await;
    println!("[DEBUG] Snapshot length: {}", reqs.len());
    assert!(reqs.iter().any(|r| matches!(r, teloxide_tests::Request::DeleteMessage(dm) if dm.chat_id == chat && dm.message_id == 31)),
        "Expected deletion of the 31st message");
    println!("[DEBUG] test_tg_flood_deletion: deletion assertion passed");

    // Check Redis stats increment
    println!("[DEBUG] Checking Redis tg:stats deleted counter");
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut conn = client.get_async_connection().await.unwrap();
    let deleted: i64 = conn.hget("tg:stats", "deleted").await.unwrap();
    println!("[DEBUG] Redis deleted count: {}", deleted);
    assert_eq!(deleted, 1);
    println!("[DEBUG] test_tg_flood_deletion completed\n");
}

#[tokio::test]
async fn test_tg_repeat_symbol_emitted() {
    println!("[DEBUG] Running test: test_tg_repeat_symbol_emitted");
    reset_redis().await;
    let bot = MockBot::new();
    Dispatcher::builder(bot.clone(), run_dispatcher())
        .build()
        .dispatch();

    let chat = ChatId(2002);
    let user = User { id: UserId(99), is_bot: false, first_name: "RepeatUser".into(), ..Default::default() };
    let text = "RepeatMe";

    println!("[DEBUG] Sending 6 identical messages to trigger TG_REPEAT");
    // Send the same message 6 times to trigger TG_REPEAT
    for _ in 0..6 {
        send_message(&bot, chat, user.clone(), text).await;
    }

    println!("[DEBUG] Calling scan_msg to inspect symbols for 'RepeatMe'");
    // Directly call scan_msg to inspect symbols
    let dummy_msg = "Dummy message to trigger scan_msg";
    let reply = scan_msg(&dummy_msg).await.unwrap();
    println!("[DEBUG] scan_msg symbols: {:?}", reply.symbols.keys());
    assert!(reply.symbols.contains_key("TG_REPEAT"), "Expected TG_REPEAT in symbols");
    println!("[DEBUG] TG_REPEAT assertion passed");

    println!("[DEBUG] Checking reputation increment in Redis for user 99");
    // Check reputation increment
    let mut conn = redis::Client::open("redis://127.0.0.1/").unwrap().get_async_connection().await.unwrap();
    let rep: i64 = conn.hget("tg:users:99", "rep").await.unwrap();
    println!("[DEBUG] Reputation for user 99: {}", rep);
    assert_eq!(rep, 1);
    println!("[DEBUG] test_tg_repeat_symbol_emitted completed\n");
}

#[tokio::test]
async fn test_tg_suspicious_deletes() {
    println!("[DEBUG] Running test: test_tg_suspicious_deletes");
    reset_redis().await;
    let bot = MockBot::new();
    Dispatcher::builder(bot.clone(), run_dispatcher())
        .build()
        .dispatch();

    let chat = ChatId(3003);
    let user_id = 123;
    println!("[DEBUG] Pre-setting reputation > 10 for user {}", user_id);
    // Pre-set high reputation >10
    let mut conn = redis::Client::open("redis://127.0.0.1/").unwrap().get_async_connection().await.unwrap();
    let _ : () = conn.hset("tg:users:123", "rep", 11).await.unwrap();
    println!("[DEBUG] Reputation set to 11");

    let user = User { id: UserId(user_id), is_bot: false, first_name: "Suspect".into(), ..Default::default() };
    send_message(&bot, chat, user.clone(), "Normal text").await;

    println!("[DEBUG] Taking snapshot to check for deletion due to TG_SUSPICIOUS");
    // Expect deletion on suspicious
    let reqs = bot.requests_snapshot().await;
    println!("[DEBUG] Snapshot length: {}", reqs.len());
    assert!(reqs.iter().any(|r| matches!(r, teloxide_tests::Request::DeleteMessage(dm) if dm.chat_id == chat)),
        "Expected deletion for TG_SUSPICIOUS");
    println!("[DEBUG] test_tg_suspicious_deletes completed\n");
}

#[tokio::test]
async fn test_makeadmin_and_selection() {
    println!("[DEBUG] Running test: test_makeadmin_and_selection");
    reset_redis().await;
    let bot = MockBot::new();
    Dispatcher::builder(bot.clone(), run_dispatcher())
        .build()
        .dispatch();

    let admin_chat = ChatId(4004);
    let admin_user = User { id: UserId(777), is_bot: false, first_name: "Admin".into(), ..Default::default() };

    println!("[DEBUG] Simulating /make_admin command in chat {} by user {}", admin_chat.0, admin_user.id);
    // Simulate /make_admin command
    let update = Update::new_message(
        Message::builder()
            .chat_id(admin_chat.0)
            .message_id(10)
            .text("/make_admin")
            .from(admin_user.clone())
            .build()
    );
    bot.send_update(update).await;
    println!("[DEBUG] Update for /make_admin sent");

    // Check Redis: admin user has this chat in their admin_chats set
    println!("[DEBUG] Verifying Redis admin_chats set for user {}", admin_user.id);
    let mut conn = redis::Client::open("redis://127.0.0.1/").unwrap().get_async_connection().await.unwrap();
    let is_member: bool = conn.sismember("777:admin_chats", admin_chat.0).await.unwrap();
    println!("[DEBUG] is_member: {}", is_member);
    assert!(is_member, "Chat should be registered as admin_chat");

    // Verify bot sent confirmation message with inline keyboard
    println!("[DEBUG] Checking bot requests for confirmation message");
    let reqs = bot.requests_snapshot().await;
    println!("[DEBUG] Snapshot length: {}", reqs.len());
    assert!(reqs.iter().any(|r| matches!(r, teloxide_tests::Request::SendMessage(sm) if sm.chat_id == admin_chat && sm.text.contains("Admin chat registered"))),
        "Expected confirmation of admin registration");
    println!("[DEBUG] test_makeadmin_and_selection completed\n");
}

#[tokio::test]
async fn test_reputation_command() {
    println!("[DEBUG] Running test: test_reputation_command");
    reset_redis().await;
    let bot = MockBot::new();
    Dispatcher::builder(bot.clone(), run_dispatcher())
        .build()
        .dispatch();

    // Pre-set user reputation key
    println!("[DEBUG] Setting reputation key tg:12345:rep = 3");
    let mut conn = redis::Client::open("redis://127.0.0.1/").unwrap().get_async_connection().await.unwrap();
    let _ : () = conn.set("tg:12345:rep", 3).await.unwrap();

    let admin_chat = ChatId(5005);
    let admin_user = User { id: UserId(777), is_bot: false, first_name: "Admin".into(), ..Default::default() };
    
    println!("[DEBUG] Simulating /reputation 12345 command");
    // Simulate /reputation 12345
    let update = Update::new_message(
        Message::builder()
            .chat_id(admin_chat.0)
            .message_id(20)
            .text("/reputation 12345")
            .from(admin_user.clone())
            .build()
    );
    bot.send_update(update).await;
    println!("[DEBUG] Update for /reputation sent");

    // Verify bot response contains correct reputation
    println!("[DEBUG] Checking bot requests for reputation response");
    let reqs = bot.requests_snapshot().await;
    println!("[DEBUG] Snapshot length: {}", reqs.len());
    assert!(reqs.iter().any(|r| matches!(r, teloxide_tests::Request::SendMessage(sm) if sm.chat_id == admin_chat && sm.text == "Reputation for 12345: 3")),
        "Expected correct reputation response");
    println!("[DEBUG] test_reputation_command completed\n");
}
