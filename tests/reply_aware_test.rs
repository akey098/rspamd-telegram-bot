use rspamd_telegram_bot::trust_manager::{TrustManager, TrustedMessageType, TrustedMessageMetadata};
use rspamd_telegram_bot::config::{key, field, symbol, reply_aware, rate_limit, selective_trust};
use rspamd_telegram_bot::handlers::scan_msg;
use teloxide::types::{Chat, ChatId, ChatKind, ChatPrivate, MediaKind, MediaText, Message, MessageCommon, MessageId, MessageKind, User, UserId};
use chrono::Utc;
use redis::Commands;
use serial_test::serial;
use std::error::Error;

/// Helper function to create a test user
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

/// Helper function to create a test chat
fn make_chat(chat_id: i64) -> Chat {
    let private_chat: ChatPrivate = ChatPrivate {
        username: Some("TestChat".into()),
        first_name: Some("TestChat".into()),
        last_name: None,
    };
    Chat {
        id: ChatId(chat_id),
        kind: ChatKind::Private(private_chat)
    }
}

/// Helper function to create a test message
fn make_message(chat_id: i64, user_id: u64, username: &str, text: &str, msg_id: u32) -> Message {
    let user = make_user(user_id, username);
    let chat = make_chat(chat_id);
    Message {
        id: MessageId(msg_id as i32),
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

/// Helper function to create a test message with reply
fn make_message_with_reply(chat_id: i64, user_id: u64, username: &str, text: &str, msg_id: u32, reply_to_msg: Message) -> Message {
    let user = make_user(user_id, username);
    let chat = make_chat(chat_id);
    Message {
        id: MessageId(msg_id as i32),
        date: Utc::now(),
        chat,
        kind: MessageKind::Common(MessageCommon {
            author_signature: None,
            effect_id: None,
            forward_origin: None,
            reply_to_message: Some(Box::new(reply_to_msg)),
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

/// Helper function to flush Redis and set up test environment
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

// ============================================================================
// UNIT TESTS FOR TRUST MANAGER
// ============================================================================

#[tokio::test]
#[serial]
async fn test_trusted_message_type_serialization() {
    // Test string conversion
    assert_eq!(TrustedMessageType::Bot.as_str(), "bot");
    assert_eq!(TrustedMessageType::Admin.as_str(), "admin");
    assert_eq!(TrustedMessageType::Verified.as_str(), "verified");
    
    // Test parsing
    assert_eq!(TrustedMessageType::from_str("bot"), Some(TrustedMessageType::Bot));
    assert_eq!(TrustedMessageType::from_str("admin"), Some(TrustedMessageType::Admin));
    assert_eq!(TrustedMessageType::from_str("verified"), Some(TrustedMessageType::Verified));
    assert_eq!(TrustedMessageType::from_str("invalid"), None);
}

#[tokio::test]
#[serial]
async fn test_trusted_message_type_scoring() {
    // Test score reductions
    assert_eq!(TrustedMessageType::Bot.score_reduction(), -3.0);
    assert_eq!(TrustedMessageType::Admin.score_reduction(), -2.0);
    assert_eq!(TrustedMessageType::Verified.score_reduction(), -1.0);
}

#[tokio::test]
#[serial]
async fn test_trusted_message_metadata_creation() {
    let metadata = TrustedMessageMetadata::new(
        MessageId(123),
        ChatId(456),
        UserId(789),
        TrustedMessageType::Bot,
    );
    
    assert_eq!(metadata.message_id.0, 123);
    assert_eq!(metadata.chat_id.0, 456);
    assert_eq!(metadata.sender_id.0, 789);
    assert_eq!(metadata.message_type, TrustedMessageType::Bot);
    
    // Test Redis key generation
    let expected_key = format!("{}{}", key::TG_TRUSTED_PREFIX, 123);
    assert_eq!(metadata.redis_key(), expected_key);
    
    let expected_metadata_key = format!("{}{}{}{}", key::TG_TRUSTED_PREFIX, 123, ":metadata", 123);
    assert_eq!(metadata.metadata_key(), expected_metadata_key);
}

#[tokio::test]
#[serial]
async fn test_trust_manager_creation() {
    // Test successful creation
    let trust_manager = TrustManager::new("redis://127.0.0.1/");
    assert!(trust_manager.is_ok());
    
    // Test failed creation with invalid URL
    let trust_manager = TrustManager::new("invalid://url");
    assert!(trust_manager.is_err());
}

#[tokio::test]
#[serial]
async fn test_trusted_message_lifecycle() {
    flush_redis();
    
    let trust_manager = TrustManager::new("redis://127.0.0.1/").unwrap();
    
    // Create test metadata
    let metadata = TrustedMessageMetadata::new(
        MessageId(999),
        ChatId(888),
        UserId(777),
        TrustedMessageType::Admin,
    );
    
    // Test marking as trusted
    let result = trust_manager.mark_trusted(metadata.clone()).await;
    assert!(result.is_ok());
    
    // Test checking if trusted
    let is_trusted = trust_manager.is_trusted(MessageId(999)).await;
    assert!(is_trusted.is_ok());
    assert!(is_trusted.unwrap());
    
    // Test getting metadata
    let retrieved_metadata = trust_manager.get_trusted_metadata(MessageId(999)).await;
    assert!(retrieved_metadata.is_ok());
    let retrieved_metadata = retrieved_metadata.unwrap();
    assert!(retrieved_metadata.is_some());
    
    let retrieved_metadata = retrieved_metadata.unwrap();
    assert_eq!(retrieved_metadata.message_id.0, 999);
    assert_eq!(retrieved_metadata.chat_id.0, 888);
    assert_eq!(retrieved_metadata.sender_id.0, 777);
    assert_eq!(retrieved_metadata.message_type, TrustedMessageType::Admin);
    
    // Test non-existent message
    let is_trusted = trust_manager.is_trusted(MessageId(999999)).await;
    assert!(is_trusted.is_ok());
    assert!(!is_trusted.unwrap());
}

#[tokio::test]
#[serial]
async fn test_reply_tracking() {
    flush_redis();
    
    let trust_manager = TrustManager::new("redis://127.0.0.1/").unwrap();
    
    // Create trusted message first
    let trusted_metadata = TrustedMessageMetadata::new(
        MessageId(111),
        ChatId(222),
        UserId(333),
        TrustedMessageType::Bot,
    );
    
    let _ = trust_manager.mark_trusted(trusted_metadata).await;
    
    // Test tracking a reply
    let result = trust_manager.track_reply(
        ChatId(222),
        MessageId(444), // reply message
        MessageId(111), // trusted message
    ).await;
    assert!(result.is_ok());
    
    // Test checking if message is reply to trusted
    let reply_info = trust_manager.is_reply_to_trusted(ChatId(222), MessageId(444)).await;
    assert!(reply_info.is_ok());
    let reply_info = reply_info.unwrap();
    assert!(reply_info.is_some());
    
    let reply_info = reply_info.unwrap();
    assert_eq!(reply_info.message_id.0, 111);
    assert_eq!(reply_info.message_type, TrustedMessageType::Bot);
}

#[tokio::test]
#[serial]
async fn test_stats_collection() {
    flush_redis();
    
    let trust_manager = TrustManager::new("redis://127.0.0.1/").unwrap();
    
    // Create some test data
    let metadata1 = TrustedMessageMetadata::new(
        MessageId(100),
        ChatId(200),
        UserId(300),
        TrustedMessageType::Bot,
    );
    
    let metadata2 = TrustedMessageMetadata::new(
        MessageId(101),
        ChatId(201),
        UserId(301),
        TrustedMessageType::Admin,
    );
    
    let _ = trust_manager.mark_trusted(metadata1).await;
    let _ = trust_manager.mark_trusted(metadata2).await;
    
    // Track some replies
    let _ = trust_manager.track_reply(ChatId(200), MessageId(400), MessageId(100)).await;
    let _ = trust_manager.track_reply(ChatId(201), MessageId(401), MessageId(101)).await;
    
    // Get stats
    let stats = trust_manager.get_stats().await;
    assert!(stats.is_ok());
    
    let stats = stats.unwrap();
    assert!(stats.trusted_messages >= 2);
    assert!(stats.reply_tracking >= 2);
}

#[tokio::test]
#[serial]
async fn test_different_trust_types() {
    flush_redis();
    
    let trust_manager = TrustManager::new("redis://127.0.0.1/").unwrap();
    
    // Test bot messages
    let bot_metadata = TrustedMessageMetadata::new(
        MessageId(500),
        ChatId(600),
        UserId(700),
        TrustedMessageType::Bot,
    );
    let _ = trust_manager.mark_trusted(bot_metadata).await;
    
    // Test admin messages
    let admin_metadata = TrustedMessageMetadata::new(
        MessageId(501),
        ChatId(601),
        UserId(701),
        TrustedMessageType::Admin,
    );
    let _ = trust_manager.mark_trusted(admin_metadata).await;
    
    // Test verified user messages
    let verified_metadata = TrustedMessageMetadata::new(
        MessageId(502),
        ChatId(602),
        UserId(702),
        TrustedMessageType::Verified,
    );
    let _ = trust_manager.mark_trusted(verified_metadata).await;
    
    // Verify all are trusted
    assert!(trust_manager.is_trusted(MessageId(500)).await.unwrap());
    assert!(trust_manager.is_trusted(MessageId(501)).await.unwrap());
    assert!(trust_manager.is_trusted(MessageId(502)).await.unwrap());
    
    // Verify metadata types
    let bot_meta = trust_manager.get_trusted_metadata(MessageId(500)).await.unwrap().unwrap();
    let admin_meta = trust_manager.get_trusted_metadata(MessageId(501)).await.unwrap().unwrap();
    let verified_meta = trust_manager.get_trusted_metadata(MessageId(502)).await.unwrap().unwrap();
    
    assert_eq!(bot_meta.message_type, TrustedMessageType::Bot);
    assert_eq!(admin_meta.message_type, TrustedMessageType::Admin);
    assert_eq!(verified_meta.message_type, TrustedMessageType::Verified);
}

#[tokio::test]
#[serial]
async fn test_cleanup_functionality() {
    flush_redis();
    
    let trust_manager = TrustManager::new("redis://127.0.0.1/").unwrap();
    
    // Test cleanup (should not error)
    let result = trust_manager.cleanup_expired().await;
    assert!(result.is_ok());
}

#[tokio::test]
#[serial]
async fn test_error_handling() {
    flush_redis();
    
    let trust_manager = TrustManager::new("redis://127.0.0.1/").unwrap();
    
    // Test with invalid message ID
    let result = trust_manager.get_trusted_metadata(MessageId(999999)).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
    
    // Test reply tracking with non-existent trusted message
    let result = trust_manager.track_reply(
        ChatId(1000),
        MessageId(1001),
        MessageId(999999), // non-existent
    ).await;
    assert!(result.is_ok()); // Should still succeed
    
    // Test reply check with non-existent message
    let result = trust_manager.is_reply_to_trusted(ChatId(1000), MessageId(999999)).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

// ============================================================================
// INTEGRATION TESTS FOR REPLY-AWARE FILTERING
// ============================================================================

#[tokio::test]
#[serial]
async fn test_reply_to_bot_message_gets_score_reduction() {
    flush_redis();
    
    // Create a trusted bot message
    let trusted_message_id = MessageId(12345);
    let chat_id = 67890;
    let user_id = 11111;
    
    let trust_manager = TrustManager::new("redis://127.0.0.1/").expect("Failed to create trust manager");
    let metadata = TrustedMessageMetadata::new(
        trusted_message_id,
        ChatId(chat_id),
        UserId(user_id),
        TrustedMessageType::Bot,
    );
    
    trust_manager.mark_trusted(metadata).await.expect("Failed to mark message as trusted");
    
    // Create reply to bot message
    let original_bot_message = make_message(chat_id, user_id, "bot", "Original bot message", trusted_message_id.0 as u32);
    let reply_message = make_message_with_reply(chat_id, 22222, "test", "This is a reply to a bot message", 1, original_bot_message);
    
    let scan_result = scan_msg(reply_message, "This is a reply to a bot message".to_string()).await;
    assert!(scan_result.is_ok(), "Scan should succeed");
    
    let scan_reply = scan_result.unwrap();
    
    // Should have TG_REPLY_BOT symbol for score reduction
    assert!(scan_reply.symbols.contains_key(symbol::TG_REPLY_BOT), 
            "Reply to bot message should have TG_REPLY_BOT symbol");
    
    // Clean up
    let client = redis::Client::open("redis://127.0.0.1/").expect("Failed to connect to Redis");
    let mut conn = client.get_connection().expect("Failed to get Redis connection");
    let _: () = conn.del(format!("{}{}", key::TG_TRUSTED_PREFIX, trusted_message_id.0)).unwrap_or_default();
}

#[tokio::test]
#[serial]
async fn test_reply_to_admin_message_gets_score_reduction() {
    flush_redis();
    
    // Create a trusted admin message
    let trusted_message_id = MessageId(12346);
    let chat_id = 67891;
    let user_id = 11112;
    
    let trust_manager = TrustManager::new("redis://127.0.0.1/").expect("Failed to create trust manager");
    let metadata = TrustedMessageMetadata::new(
        trusted_message_id,
        ChatId(chat_id),
        UserId(user_id),
        TrustedMessageType::Admin,
    );
    
    trust_manager.mark_trusted(metadata).await.expect("Failed to mark message as trusted");
    
    // Create reply to admin message
    let original_admin_message = make_message(chat_id, user_id, "admin", "Original admin message", trusted_message_id.0 as u32);
    let reply_message = make_message_with_reply(chat_id, 22223, "test", "This is a reply to an admin message", 1, original_admin_message);
    
    let scan_result = scan_msg(reply_message, "This is a reply to an admin message".to_string()).await;
    assert!(scan_result.is_ok(), "Scan should succeed");
    
    let scan_reply = scan_result.unwrap();
    
    // Should have TG_REPLY_ADMIN symbol for score reduction
    assert!(scan_reply.symbols.contains_key(symbol::TG_REPLY_ADMIN), 
            "Reply to admin message should have TG_REPLY_ADMIN symbol");
    
    // Clean up
    let client = redis::Client::open("redis://127.0.0.1/").expect("Failed to connect to Redis");
    let mut conn = client.get_connection().expect("Failed to get Redis connection");
    let _: () = conn.del(format!("{}{}", key::TG_TRUSTED_PREFIX, trusted_message_id.0)).unwrap_or_default();
}

#[tokio::test]
#[serial]
async fn test_reply_to_verified_user_message_gets_score_reduction() {
    flush_redis();
    
    // Create a trusted verified user message
    let trusted_message_id = MessageId(12347);
    let chat_id = 67892;
    let user_id = 11113;
    
    let trust_manager = TrustManager::new("redis://127.0.0.1/").expect("Failed to create trust manager");
    let metadata = TrustedMessageMetadata::new(
        trusted_message_id,
        ChatId(chat_id),
        UserId(user_id),
        TrustedMessageType::Verified,
    );
    
    trust_manager.mark_trusted(metadata).await.expect("Failed to mark message as trusted");
    
    // Create reply to verified user message
    let original_verified_message = make_message(chat_id, user_id, "verified", "Original verified user message", trusted_message_id.0 as u32);
    let reply_message = make_message_with_reply(chat_id, 22224, "test", "This is a reply to a verified user message", 1, original_verified_message);
    
    let scan_result = scan_msg(reply_message, "This is a reply to a verified user message".to_string()).await;
    assert!(scan_result.is_ok(), "Scan should succeed");
    
    let scan_reply = scan_result.unwrap();
    
    // Should have TG_REPLY_VERIFIED symbol for score reduction
    assert!(scan_reply.symbols.contains_key(symbol::TG_REPLY_VERIFIED), 
            "Reply to verified user message should have TG_REPLY_VERIFIED symbol");
    
    // Clean up
    let client = redis::Client::open("redis://127.0.0.1/").expect("Failed to connect to Redis");
    let mut conn = client.get_connection().expect("Failed to get Redis connection");
    let _: () = conn.del(format!("{}{}", key::TG_TRUSTED_PREFIX, trusted_message_id.0)).unwrap_or_default();
}

#[tokio::test]
#[serial]
async fn test_regular_message_does_not_get_reply_symbols() {
    flush_redis();
    
    let chat_id = 67893;
    let user_id = 22225;
    
    // Create a regular message (not a reply)
    let regular_message = make_message(chat_id, user_id, "test", "This is a regular message", 1);
    
    let scan_result = scan_msg(regular_message, "This is a regular message".to_string()).await;
    assert!(scan_result.is_ok(), "Scan should succeed");
    
    let scan_reply = scan_result.unwrap();
    
    // Should not have any reply symbols
    assert!(!scan_reply.symbols.contains_key(symbol::TG_REPLY_BOT), 
            "Regular message should not have TG_REPLY_BOT symbol");
    assert!(!scan_reply.symbols.contains_key(symbol::TG_REPLY_ADMIN), 
            "Regular message should not have TG_REPLY_ADMIN symbol");
    assert!(!scan_reply.symbols.contains_key(symbol::TG_REPLY_VERIFIED), 
            "Regular message should not have TG_REPLY_VERIFIED symbol");
}

#[tokio::test]
#[serial]
async fn test_reply_to_non_trusted_message_does_not_get_score_reduction() {
    flush_redis();
    
    let chat_id = 67894;
    let user_id = 22226;
    
    // Create a non-trusted message
    let non_trusted_message_id = MessageId(54321);
    let original_non_trusted_message = make_message(chat_id, 55555, "user", "Original non-trusted message", non_trusted_message_id.0 as u32);
    let reply_to_non_trusted = make_message_with_reply(chat_id, user_id, "test", "This is a reply to a non-trusted message", 1, original_non_trusted_message);
    
    let scan_result = scan_msg(reply_to_non_trusted, "This is a reply to a non-trusted message".to_string()).await;
    assert!(scan_result.is_ok(), "Scan should succeed");
    
    let scan_reply = scan_result.unwrap();
    
    // Should not have reply score reduction symbols
    assert!(!scan_reply.symbols.contains_key(symbol::TG_REPLY_BOT), 
            "Reply to non-trusted message should not have TG_REPLY_BOT symbol");
    assert!(!scan_reply.symbols.contains_key(symbol::TG_REPLY_ADMIN), 
            "Reply to non-trusted message should not have TG_REPLY_ADMIN symbol");
    assert!(!scan_reply.symbols.contains_key(symbol::TG_REPLY_VERIFIED), 
            "Reply to non-trusted message should not have TG_REPLY_VERIFIED symbol");
}

#[tokio::test]
#[serial]
async fn test_reply_with_spam_content_still_gets_detected() {
    flush_redis();
    
    // Create a trusted message
    let trusted_message_id = MessageId(12348);
    let chat_id = 67895;
    let user_id = 11114;
    
    let trust_manager = TrustManager::new("redis://127.0.0.1/").expect("Failed to create trust manager");
    let metadata = TrustedMessageMetadata::new(
        trusted_message_id,
        ChatId(chat_id),
        UserId(user_id),
        TrustedMessageType::Bot,
    );
    
    trust_manager.mark_trusted(metadata).await.expect("Failed to mark message as trusted");
    
    // Create reply with spam content (excessive links)
    let original_bot_message = make_message(chat_id, user_id, "bot", "Original bot message", trusted_message_id.0 as u32);
    let spam_reply = make_message_with_reply(chat_id, 22227, "test", "Check out these links: https://example1.com https://example2.com https://example3.com https://example4.com", 1, original_bot_message);
    
    let scan_result = scan_msg(spam_reply, "Check out these links: https://example1.com https://example2.com https://example3.com https://example4.com".to_string()).await;
    assert!(scan_result.is_ok(), "Scan should succeed");
    
    let scan_reply = scan_result.unwrap();
    
    // Should have both reply symbol and spam detection symbol
    assert!(scan_reply.symbols.contains_key(symbol::TG_REPLY_BOT), 
            "Reply to bot message should have TG_REPLY_BOT symbol");
    assert!(scan_reply.symbols.contains_key(symbol::TG_LINK_SPAM), 
            "Reply with excessive links should have TG_LINK_SPAM symbol");
    
    // Clean up
    let client = redis::Client::open("redis://127.0.0.1/").expect("Failed to connect to Redis");
    let mut conn = client.get_connection().expect("Failed to get Redis connection");
    let _: () = conn.del(format!("{}{}", key::TG_TRUSTED_PREFIX, trusted_message_id.0)).unwrap_or_default();
}

#[tokio::test]
#[serial]
async fn test_reply_with_invite_link_still_gets_detected() {
    flush_redis();
    
    // Create a trusted message
    let trusted_message_id = MessageId(12349);
    let chat_id = 67896;
    let user_id = 11115;
    
    let trust_manager = TrustManager::new("redis://127.0.0.1/").expect("Failed to create trust manager");
    let metadata = TrustedMessageMetadata::new(
        trusted_message_id,
        ChatId(chat_id),
        UserId(user_id),
        TrustedMessageType::Bot,
    );
    
    trust_manager.mark_trusted(metadata).await.expect("Failed to mark message as trusted");
    
    // Create reply with invite link
    let original_bot_message = make_message(chat_id, user_id, "bot", "Original bot message", trusted_message_id.0 as u32);
    let invite_spam_reply = make_message_with_reply(chat_id, 22228, "test", "Join our group: t.me/joinchat/abc123", 1, original_bot_message);
    
    let scan_result = scan_msg(invite_spam_reply, "Join our group: t.me/joinchat/abc123".to_string()).await;
    assert!(scan_result.is_ok(), "Scan should succeed");
    
    let scan_reply = scan_result.unwrap();
    
    assert!(scan_reply.symbols.contains_key(symbol::TG_REPLY_BOT), 
            "Reply to bot message should have TG_REPLY_BOT symbol");
    assert!(scan_reply.symbols.contains_key(symbol::TG_INVITE_LINK), 
            "Reply with invite link should have TG_INVITE_LINK symbol");
    
    // Clean up
    let client = redis::Client::open("redis://127.0.0.1/").expect("Failed to connect to Redis");
    let mut conn = client.get_connection().expect("Failed to get Redis connection");
    let _: () = conn.del(format!("{}{}", key::TG_TRUSTED_PREFIX, trusted_message_id.0)).unwrap_or_default();
}

#[tokio::test]
#[serial]
async fn test_multiple_replies_to_same_trusted_message() {
    flush_redis();
    
    // Create a trusted message
    let trusted_message_id = MessageId(12350);
    let chat_id = 67897;
    let user_id = 11116;
    
    let trust_manager = TrustManager::new("redis://127.0.0.1/").expect("Failed to create trust manager");
    let metadata = TrustedMessageMetadata::new(
        trusted_message_id,
        ChatId(chat_id),
        UserId(user_id),
        TrustedMessageType::Admin,
    );
    
    trust_manager.mark_trusted(metadata).await.expect("Failed to mark message as trusted");
    
    // Create multiple replies to the same trusted message
    let original_admin_message = make_message(chat_id, user_id, "admin", "Original admin message", trusted_message_id.0 as u32);
    
    for i in 1..=3 {
        let reply_message = make_message_with_reply(chat_id, 22229 + i as u64, "test", &format!("Reply {} to admin message", i), i, original_admin_message.clone());
        
        let scan_result = scan_msg(reply_message, format!("Reply {} to admin message", i)).await;
        assert!(scan_result.is_ok(), "Scan should succeed");
        
        let scan_reply = scan_result.unwrap();
        
        // Each reply should get the reply symbol
        assert!(scan_reply.symbols.contains_key(symbol::TG_REPLY_ADMIN), 
                "Reply {} to admin message should have TG_REPLY_ADMIN symbol", i);
    }
    
    // Clean up
    let client = redis::Client::open("redis://127.0.0.1/").expect("Failed to connect to Redis");
    let mut conn = client.get_connection().expect("Failed to get Redis connection");
    let _: () = conn.del(format!("{}{}", key::TG_TRUSTED_PREFIX, trusted_message_id.0)).unwrap_or_default();
}

#[tokio::test]
#[serial]
async fn test_trusted_message_expiration() {
    flush_redis();
    
    // Create a trusted message with short TTL
    let trusted_message_id = MessageId(12351);
    let chat_id = 67898;
    let user_id = 11117;
    
    let trust_manager = TrustManager::new("redis://127.0.0.1/").expect("Failed to create trust manager");
    let metadata = TrustedMessageMetadata::new(
        trusted_message_id,
        ChatId(chat_id),
        UserId(user_id),
        TrustedMessageType::Bot,
    );
    
    trust_manager.mark_trusted(metadata).await.expect("Failed to mark message as trusted");
    
    // Verify message is trusted initially
    assert!(trust_manager.is_trusted(MessageId(12351)).await.unwrap());
    
    // Note: In a real test, we would wait for the TTL to expire
    // For this test, we'll manually delete the trusted message to simulate expiration
    let client = redis::Client::open("redis://127.0.0.1/").expect("Failed to connect to Redis");
    let mut conn = client.get_connection().expect("Failed to get Redis connection");
    let _: () = conn.del(format!("{}{}", key::TG_TRUSTED_PREFIX, trusted_message_id.0)).unwrap_or_default();
    
    // Verify message is no longer trusted
    assert!(!trust_manager.is_trusted(MessageId(12351)).await.unwrap());
    
    // Create reply to expired trusted message
    let original_bot_message = make_message(chat_id, user_id, "bot", "Original bot message", trusted_message_id.0 as u32);
    let reply_message = make_message_with_reply(chat_id, 22230, "test", "This is a reply to an expired trusted message", 1, original_bot_message);
    
    let scan_result = scan_msg(reply_message, "This is a reply to an expired trusted message".to_string()).await;
    assert!(scan_result.is_ok(), "Scan should succeed");
    
    let scan_reply = scan_result.unwrap();
    
    // Should not have reply symbol since trusted message expired
    assert!(!scan_reply.symbols.contains_key(symbol::TG_REPLY_BOT), 
            "Reply to expired trusted message should not have TG_REPLY_BOT symbol");
}

#[tokio::test]
#[serial]
async fn test_different_trust_levels_score_reduction() {
    flush_redis();
    
    let chat_id = 67899;
    let user_id = 11118;
    
    let trust_manager = TrustManager::new("redis://127.0.0.1/").expect("Failed to create trust manager");
    
    // Test different trust levels
    let test_cases = vec![
        (MessageId(1001), TrustedMessageType::Bot, symbol::TG_REPLY_BOT),
        (MessageId(1002), TrustedMessageType::Admin, symbol::TG_REPLY_ADMIN),
        (MessageId(1003), TrustedMessageType::Verified, symbol::TG_REPLY_VERIFIED),
    ];
    
    for (message_id, trust_type, expected_symbol) in test_cases.clone() {
        // Mark message as trusted
        let metadata = TrustedMessageMetadata::new(
            message_id,
            ChatId(chat_id),
            UserId(user_id),
            trust_type.clone(),
        );
        
        trust_manager.mark_trusted(metadata).await.expect("Failed to mark message as trusted");
        
        // Create reply to this trusted message
        let original_message = make_message(chat_id, user_id, "user", "Original trusted message", message_id.0 as u32);
        let reply_message = make_message_with_reply(chat_id, 22231, "test", "This is a reply to a trusted message", 1, original_message);
        
        let scan_result = scan_msg(reply_message, "This is a reply to a trusted message".to_string()).await;
        assert!(scan_result.is_ok(), "Scan should succeed");
        
        let scan_reply = scan_result.unwrap();
        
        // Should have the appropriate reply symbol
        assert!(scan_reply.symbols.contains_key(expected_symbol), 
                "Reply to {} message should have {} symbol", trust_type.clone().as_str(), expected_symbol);
    }
    
    // Clean up
    let client = redis::Client::open("redis://127.0.0.1/").expect("Failed to connect to Redis");
    let mut conn = client.get_connection().expect("Failed to get Redis connection");
    for (message_id, _, _) in test_cases {
        let _: () = conn.del(format!("{}{}", key::TG_TRUSTED_PREFIX, message_id.0)).unwrap_or_default();
    }
}

#[tokio::test]
#[serial]
async fn test_reply_aware_filtering_performance() {
    flush_redis();
    
    let trust_manager = TrustManager::new("redis://127.0.0.1/").expect("Failed to create trust manager");
    let chat_id = 67900;
    let user_id = 11119;
    
    // Create multiple trusted messages
    let trusted_messages: Vec<MessageId> = (1..=10).map(|i| MessageId(20000 + i)).collect();
    
    for (i, message_id) in trusted_messages.iter().enumerate() {
        let metadata = TrustedMessageMetadata::new(
            *message_id,
            ChatId(chat_id),
            UserId(user_id),
            TrustedMessageType::Bot,
        );
        
        trust_manager.mark_trusted(metadata).await.expect("Failed to mark message as trusted");
    }
    
    // Test performance with multiple replies
    let start_time = std::time::Instant::now();
    
    for (i, trusted_message_id) in trusted_messages.iter().enumerate() {
        let original_message = make_message(chat_id, user_id, "bot", "Original bot message", trusted_message_id.0 as u32);
        let reply_message = make_message_with_reply(chat_id, 22232 + i as u64, "test", &format!("Reply {}", i), i as u32, original_message);
        
        let scan_result = scan_msg(reply_message, format!("Reply {}", i)).await;
        assert!(scan_result.is_ok(), "Scan should succeed");
        
        let scan_reply = scan_result.unwrap();
        assert!(scan_reply.symbols.contains_key(symbol::TG_REPLY_BOT), 
                "Reply {} should have TG_REPLY_BOT symbol", i);
    }
    
    let duration = start_time.elapsed();
    println!("Processed {} replies in {:?}", trusted_messages.len(), duration);
    
    // Performance should be reasonable (less than 1 second for 10 messages)
    assert!(duration.as_millis() < 1000, "Performance test took too long: {:?}", duration);
    
    // Clean up
    let client = redis::Client::open("redis://127.0.0.1/").expect("Failed to connect to Redis");
    let mut conn = client.get_connection().expect("Failed to get Redis connection");
    for trusted_message_id in trusted_messages {
        let _: () = conn.del(format!("{}{}", key::TG_TRUSTED_PREFIX, trusted_message_id.0)).unwrap_or_default();
    }
}

#[tokio::test]
#[serial]
async fn test_reply_aware_filtering_edge_cases() {
    flush_redis();
    
    let trust_manager = TrustManager::new("redis://127.0.0.1/").expect("Failed to create trust manager");
    let chat_id = 67901;
    let user_id = 11120;
    
    // Test 1: Reply to a message that was trusted but then untrusted
    let trusted_message_id = MessageId(30001);
    let metadata = TrustedMessageMetadata::new(
        trusted_message_id,
        ChatId(chat_id),
        UserId(user_id),
        TrustedMessageType::Bot,
    );
    
    trust_manager.mark_trusted(metadata).await.expect("Failed to mark message as trusted");
    
    // Manually untrust the message
    let client = redis::Client::open("redis://127.0.0.1/").expect("Failed to connect to Redis");
    let mut conn = client.get_connection().expect("Failed to get Redis connection");
    let _: () = conn.del(format!("{}{}", key::TG_TRUSTED_PREFIX, trusted_message_id.0)).unwrap_or_default();
    
    // Create reply to untrusted message
    let original_message = make_message(chat_id, user_id, "bot", "Original bot message", trusted_message_id.0 as u32);
    let reply_message = make_message_with_reply(chat_id, 22240, "test", "Reply to untrusted message", 1, original_message);
    
    let scan_result = scan_msg(reply_message, "Reply to untrusted message".to_string()).await;
    assert!(scan_result.is_ok(), "Scan should succeed");
    
    let scan_reply = scan_result.unwrap();
    
    // Should not have reply symbol since message is no longer trusted
    assert!(!scan_reply.symbols.contains_key(symbol::TG_REPLY_BOT), 
            "Reply to untrusted message should not have TG_REPLY_BOT symbol");
    
    // Test 2: Reply to a message with invalid metadata
    let invalid_message_id = MessageId(30002);
    let original_invalid_message = make_message(chat_id, user_id, "bot", "Original bot message", invalid_message_id.0 as u32);
    let reply_to_invalid = make_message_with_reply(chat_id, 22241, "test", "Reply to invalid message", 2, original_invalid_message);
    
    let scan_result = scan_msg(reply_to_invalid, "Reply to invalid message".to_string()).await;
    assert!(scan_result.is_ok(), "Scan should succeed");
    
    let scan_reply = scan_result.unwrap();
    
    // Should not have reply symbol since there's no trusted metadata
    assert!(!scan_reply.symbols.contains_key(symbol::TG_REPLY_BOT), 
            "Reply to invalid message should not have TG_REPLY_BOT symbol");
}

#[tokio::test]
#[serial]
async fn test_reply_aware_filtering_integration_with_existing_symbols() {
    flush_redis();
    
    // Create a trusted message
    let trusted_message_id = MessageId(40001);
    let chat_id = 67902;
    let user_id = 11121;
    
    let trust_manager = TrustManager::new("redis://127.0.0.1/").expect("Failed to create trust manager");
    let metadata = TrustedMessageMetadata::new(
        trusted_message_id,
        ChatId(chat_id),
        UserId(user_id),
        TrustedMessageType::Bot,
    );
    
    trust_manager.mark_trusted(metadata).await.expect("Failed to mark message as trusted");
    
    // Create reply with multiple spam indicators
    let original_bot_message = make_message(chat_id, user_id, "bot", "Original bot message", trusted_message_id.0 as u32);
    let spam_reply = make_message_with_reply(
        chat_id, 
        22242, 
        "test", 
        "HELLO EVERYONE! 😀😃😄😁😆😅😂🤣😊😇🙂🙃 @user1 @user2 @user3 @user4 @user5 @user6 Check out: https://t.me/joinchat/ABC123 https://bit.ly/deal", 
        1, 
        original_bot_message
    );
    
    let scan_result = scan_msg(spam_reply, "HELLO EVERYONE! 😀😃😄😁😆😅😂🤣😊😇🙂🙃 @user1 @user2 @user3 @user4 @user5 @user6 Check out: https://t.me/joinchat/ABC123 https://bit.ly/deal".to_string()).await;
    assert!(scan_result.is_ok(), "Scan should succeed");
    
    let scan_reply = scan_result.unwrap();
    
    // Should have both reply symbol and existing spam detection symbols
    assert!(scan_reply.symbols.contains_key(symbol::TG_REPLY_BOT), 
            "Reply to bot message should have TG_REPLY_BOT symbol");
    
    // Should also have existing spam detection symbols
    let triggered_symbols: Vec<&str> = scan_reply.symbols.keys().map(|s| s.as_str()).collect();
    assert!(triggered_symbols.contains(&symbol::TG_CAPS), "Expected TG_CAPS");
    assert!(triggered_symbols.contains(&symbol::TG_EMOJI_SPAM), "Expected TG_EMOJI_SPAM");
    assert!(triggered_symbols.contains(&symbol::TG_MENTIONS), "Expected TG_MENTIONS");
    assert!(triggered_symbols.contains(&symbol::TG_INVITE_LINK), "Expected TG_INVITE_LINK");
    assert!(triggered_symbols.contains(&symbol::TG_SHORTENER), "Expected TG_SHORTENER");
    
    // Verify we have at least 6 symbols triggered (reply + 5 spam symbols)
    assert!(triggered_symbols.len() >= 6, "Expected at least 6 symbols to be triggered");
    
    // Clean up
    let client = redis::Client::open("redis://127.0.0.1/").expect("Failed to connect to Redis");
    let mut conn = client.get_connection().expect("Failed to get Redis connection");
    let _: () = conn.del(format!("{}{}", key::TG_TRUSTED_PREFIX, trusted_message_id.0)).unwrap_or_default();
}

#[tokio::test]
async fn test_rate_limiting() -> Result<(), Box<dyn Error + Send + Sync>> {
    let trust_manager = TrustManager::new("redis://127.0.0.1/")?;
    let test_user_id = UserId(123456789);
    
    // Clean up any existing test data by creating a new connection
    let redis_client = redis::Client::open("redis://127.0.0.1/")?;
    let mut conn = redis_client.get_connection()?;
    let trusted_rate_key = format!("{}{}", rate_limit::TRUSTED_MESSAGE_RATE_PREFIX, test_user_id.0);
    let reply_rate_key = format!("{}{}", rate_limit::REPLY_RATE_PREFIX, test_user_id.0);
    let _: () = conn.del(&trusted_rate_key)?;
    let _: () = conn.del(&reply_rate_key)?;
    
    // Test trusted message rate limiting
    for i in 0..reply_aware::MAX_TRUSTED_MESSAGES_PER_HOUR + 1 {
        let can_create = trust_manager.can_create_trusted_message(test_user_id).await?;
        if i < reply_aware::MAX_TRUSTED_MESSAGES_PER_HOUR {
            assert!(can_create, "Should be able to create trusted message {}", i);
        } else {
            assert!(!can_create, "Should be rate limited after {} messages", i);
        }
    }
    
    // Test reply rate limiting
    for i in 0..reply_aware::MAX_REPLIES_PER_HOUR + 1 {
        let can_reply = trust_manager.can_reply_to_trusted(test_user_id).await?;
        if i < reply_aware::MAX_REPLIES_PER_HOUR {
            assert!(can_reply, "Should be able to reply {}", i);
        } else {
            assert!(!can_reply, "Should be rate limited after {} replies", i);
        }
    }
    
    println!("Rate limiting tests passed");
    
    // Clean up
    let _: () = conn.del(&trusted_rate_key)?;
    let _: () = conn.del(&reply_rate_key)?;
    
    Ok(())
}

#[tokio::test]
async fn test_selective_trusting() -> Result<(), Box<dyn Error + Send + Sync>> {
    let trust_manager = TrustManager::new("redis://127.0.0.1/")?;
    
    // Test bot message trusting
    let bot_metadata = TrustedMessageMetadata::new(
        MessageId(1),
        ChatId(100),
        UserId(999), // Bot user ID
        TrustedMessageType::Bot,
    );
    
    let should_trust = trust_manager.should_trust_message(&bot_metadata).await?;
    assert_eq!(should_trust, selective_trust::TRUST_BOT_MESSAGES);
    
    // Test admin message trusting
    let admin_metadata = TrustedMessageMetadata::new(
        MessageId(2),
        ChatId(100),
        UserId(888),
        TrustedMessageType::Admin,
    );
    
    let should_trust = trust_manager.should_trust_message(&admin_metadata).await?;
    assert_eq!(should_trust, selective_trust::TRUST_ADMIN_MESSAGES);
    
    // Test verified user message trusting
    let verified_metadata = TrustedMessageMetadata::new(
        MessageId(3),
        ChatId(100),
        UserId(777),
        TrustedMessageType::Verified,
    );
    
    let should_trust = trust_manager.should_trust_message(&verified_metadata).await?;
    assert_eq!(should_trust, selective_trust::TRUST_VERIFIED_MESSAGES);
    
    println!("Selective trusting tests passed");
    
    Ok(())
}

#[tokio::test]
async fn test_anti_evasion_patterns() -> Result<(), Box<dyn Error + Send + Sync>> {
    let trust_manager = TrustManager::new("redis://127.0.0.1/")?;
    let test_user_id = UserId(123456789);
    
    // Clean up any existing test data by creating a new connection
    let redis_client = redis::Client::open("redis://127.0.0.1/")?;
    let mut conn = redis_client.get_connection()?;
    let spam_key = format!("{}{}", rate_limit::SPAM_PATTERN_PREFIX, test_user_id.0);
    let _: () = conn.del(&spam_key)?;
    
    // Test excessive links
    let text_with_links = "Check out these links: http://example1.com https://example2.com http://example3.com";
    let patterns = trust_manager.check_reply_spam_patterns(text_with_links, test_user_id).await?;
    assert!(patterns.contains(&"TG_REPLY_LINK_SPAM".to_string()));
    
    // Test phone numbers
    let text_with_phones = "Call me at +1234567890 or +9876543210";
    let patterns = trust_manager.check_reply_spam_patterns(text_with_phones, test_user_id).await?;
    assert!(patterns.contains(&"TG_REPLY_PHONE_SPAM".to_string()));
    
    // Test invite links
    let text_with_invites = "Join our group: t.me/joinchat/abc123";
    let patterns = trust_manager.check_reply_spam_patterns(text_with_invites, test_user_id).await?;
    assert!(patterns.contains(&"TG_REPLY_INVITE_SPAM".to_string()));
    
    // Test excessive caps
    let text_with_caps = "THIS IS A MESSAGE WITH TOO MANY CAPITAL LETTERS";
    let patterns = trust_manager.check_reply_spam_patterns(text_with_caps, test_user_id).await?;
    assert!(patterns.contains(&"TG_REPLY_CAPS_SPAM".to_string()));
    
    // Test normal text (should not trigger patterns)
    let normal_text = "This is a normal message with some links: http://example.com";
    let patterns = trust_manager.check_reply_spam_patterns(normal_text, test_user_id).await?;
    assert!(patterns.is_empty());
    
    println!("Anti-evasion pattern tests passed");
    
    // Clean up
    let _: () = conn.del(&spam_key)?;
    
    Ok(())
}

#[tokio::test]
async fn test_spam_pattern_tracking() -> Result<(), Box<dyn Error + Send + Sync>> {
    let trust_manager = TrustManager::new("redis://127.0.0.1/")?;
    let test_user_id = UserId(123456789);
    
    // Clean up any existing test data by creating a new connection
    let redis_client = redis::Client::open("redis://127.0.0.1/")?;
    let mut conn = redis_client.get_connection()?;
    let spam_key = format!("{}{}", rate_limit::SPAM_PATTERN_PREFIX, test_user_id.0);
    let _: () = conn.del(&spam_key)?;
    
    // Track some spam patterns
    let patterns = vec!["TG_REPLY_LINK_SPAM".to_string(), "TG_REPLY_PHONE_SPAM".to_string()];
    trust_manager.track_spam_patterns(test_user_id, &patterns).await?;
    
    // Retrieve patterns
    let retrieved_patterns = trust_manager.get_spam_patterns(test_user_id).await?;
    assert_eq!(retrieved_patterns.len(), 2);
    assert!(retrieved_patterns.contains(&"TG_REPLY_LINK_SPAM".to_string()));
    assert!(retrieved_patterns.contains(&"TG_REPLY_PHONE_SPAM".to_string()));
    
    println!("Spam pattern tracking tests passed");
    
    // Clean up
    let _: () = conn.del(&spam_key)?;
    
    Ok(())
}

#[tokio::test]
async fn test_score_reduction_calculation() -> Result<(), Box<dyn Error + Send + Sync>> {
    let trust_manager = TrustManager::new("redis://127.0.0.1/")?;
    let test_user_id = UserId(123456789);
    
    // Clean up any existing test data by creating a new connection
    let redis_client = redis::Client::open("redis://127.0.0.1/")?;
    let mut conn = redis_client.get_connection()?;
    let spam_key = format!("{}{}", rate_limit::SPAM_PATTERN_PREFIX, test_user_id.0);
    let _: () = conn.del(&spam_key)?;
    
    // Test bot message score reduction
    let bot_metadata = TrustedMessageMetadata::new(
        MessageId(1),
        ChatId(100),
        UserId(999),
        TrustedMessageType::Bot,
    );
    
    let reduction = trust_manager.calculate_score_reduction(&bot_metadata, test_user_id).await?;
    assert_eq!(reduction, reply_aware::trust_levels::BOT_TRUST_LEVEL);
    
    // Test with spam patterns (should reduce the reduction)
    let patterns = vec!["TG_REPLY_LINK_SPAM".to_string()];
    trust_manager.track_spam_patterns(test_user_id, &patterns).await?;
    
    let reduction_with_spam = trust_manager.calculate_score_reduction(&bot_metadata, test_user_id).await?;
    assert!(reduction_with_spam > reduction, "Score reduction should be reduced for users with spam patterns");
    
    println!("Score reduction calculation tests passed");
    
    // Clean up
    let _: () = conn.del(&spam_key)?;
    
    Ok(())
}

#[tokio::test]
async fn test_advanced_trusted_message_creation() -> Result<(), Box<dyn Error + Send + Sync>> {
    let trust_manager = TrustManager::new("redis://127.0.0.1/")?;
    let test_user_id = UserId(123456789);
    
    // Clean up any existing test data by creating a new connection
    let redis_client = redis::Client::open("redis://127.0.0.1/")?;
    let mut conn = redis_client.get_connection()?;
    let trusted_rate_key = format!("{}{}", rate_limit::TRUSTED_MESSAGE_RATE_PREFIX, test_user_id.0);
    let _: () = conn.del(&trusted_rate_key)?;
    
    // Test successful trusted message creation
    let metadata = TrustedMessageMetadata::new(
        MessageId(100),
        ChatId(200),
        test_user_id,
        TrustedMessageType::Bot,
    );
    
    let success = trust_manager.mark_trusted_advanced(metadata).await?;
    assert!(success, "Should be able to create trusted message");
    
    // Verify the message is trusted
    let is_trusted = trust_manager.is_trusted(MessageId(100)).await?;
    assert!(is_trusted, "Message should be marked as trusted");
    
    // Test rate limiting
    for _ in 0..reply_aware::MAX_TRUSTED_MESSAGES_PER_HOUR {
        let metadata = TrustedMessageMetadata::new(
            MessageId(101),
            ChatId(200),
            test_user_id,
            TrustedMessageType::Bot,
        );
        let success = trust_manager.mark_trusted_advanced(metadata).await?;
        assert!(success, "Should be able to create trusted messages within limit");
    }
    
    // Test that rate limiting prevents further creation
    let metadata = TrustedMessageMetadata::new(
        MessageId(102),
        ChatId(200),
        test_user_id,
        TrustedMessageType::Bot,
    );
    let success = trust_manager.mark_trusted_advanced(metadata).await?;
    assert!(!success, "Should be rate limited");
    
    println!("Advanced trusted message creation tests passed");
    
    // Clean up
    let _: () = conn.del(&trusted_rate_key)?;
    
    Ok(())
}

#[tokio::test]
async fn test_reputation_integration() -> Result<(), Box<dyn Error + Send + Sync>> {
    let trust_manager = TrustManager::new("redis://127.0.0.1/")?;
    let test_user_id = UserId(123456789);
    
    // Clean up any existing test data by creating a new connection
    let redis_client = redis::Client::open("redis://127.0.0.1/")?;
    let mut conn = redis_client.get_connection()?;
    let reputation_key = format!("tg:reputation:user:{}", test_user_id.0);
    let _: () = conn.del(&reputation_key)?;
    
    // Set up test reputation
    let _: () = conn.hset(&reputation_key, "bad", 5)?;
    let _: () = conn.hset(&reputation_key, "good", 10)?;
    
    // Test reputation retrieval
    let reputation = trust_manager.get_user_reputation(test_user_id).await?;
    assert_eq!(reputation, -5); // bad - good = 5 - 10 = -5 (good reputation)
    
    // Test selective trusting with good reputation
    let metadata = TrustedMessageMetadata::new(
        MessageId(200),
        ChatId(300),
        test_user_id,
        TrustedMessageType::Verified,
    );
    
    let should_trust = trust_manager.should_trust_message(&metadata).await?;
    // Should trust if reputation is better than minimum threshold
    let expected_trust = reputation >= selective_trust::MIN_REPUTATION_FOR_TRUST;
    assert_eq!(should_trust, expected_trust);
    
    println!("Reputation integration tests passed");
    
    // Clean up
    let _: () = conn.del(&reputation_key)?;
    
    Ok(())
}

#[tokio::test]
async fn test_message_age_filtering() -> Result<(), Box<dyn Error + Send + Sync>> {
    let trust_manager = TrustManager::new("redis://127.0.0.1/")?;
    
    // Test recent message (should be trusted)
    let recent_metadata = TrustedMessageMetadata::new(
        MessageId(300),
        ChatId(400),
        UserId(555),
        TrustedMessageType::Admin,
    );
    
    let should_trust_recent = trust_manager.should_trust_message(&recent_metadata).await?;
    assert_eq!(should_trust_recent, selective_trust::TRUST_RECENT_MESSAGES_ONLY);
    
    // Test old message (should not be trusted if TRUST_RECENT_MESSAGES_ONLY is true)
    let old_timestamp = Utc::now() - chrono::Duration::seconds(selective_trust::MAX_TRUSTED_MESSAGE_AGE as i64 + 3600);
    let mut old_metadata = TrustedMessageMetadata::new(
        MessageId(301),
        ChatId(400),
        UserId(555),
        TrustedMessageType::Admin,
    );
    old_metadata.timestamp = old_timestamp;
    
    let should_trust_old = trust_manager.should_trust_message(&old_metadata).await?;
    if selective_trust::TRUST_RECENT_MESSAGES_ONLY {
        assert!(!should_trust_old, "Old messages should not be trusted");
    }
    
    println!("Message age filtering tests passed");
    
    Ok(())
}

#[tokio::test]
async fn test_configuration_constants() -> Result<(), Box<dyn Error + Send + Sync>> {
    // Test that configuration constants are reasonable
    assert!(reply_aware::MAX_TRUSTED_MESSAGES_PER_HOUR > 0);
    assert!(reply_aware::MAX_REPLIES_PER_HOUR > 0);
    assert!(reply_aware::MAX_SCORE_REDUCTION < 0.0);
    assert!(reply_aware::MIN_SPAM_SCORE_IN_REPLIES > 0.0);
    
    // Test trust levels
    assert!(reply_aware::trust_levels::BOT_TRUST_LEVEL < reply_aware::trust_levels::ADMIN_TRUST_LEVEL);
    assert!(reply_aware::trust_levels::ADMIN_TRUST_LEVEL < reply_aware::trust_levels::VERIFIED_TRUST_LEVEL);
    assert!(reply_aware::trust_levels::VERIFIED_TRUST_LEVEL < reply_aware::trust_levels::REGULAR_TRUST_LEVEL);
    
    // Test anti-evasion thresholds
    assert!(reply_aware::anti_evasion::MAX_LINKS_IN_REPLY > 0);
    assert!(reply_aware::anti_evasion::MAX_CAPS_RATIO_IN_REPLY > 0.0);
    assert!(reply_aware::anti_evasion::MAX_CAPS_RATIO_IN_REPLY < 1.0);
    
    // Test selective trust settings
    assert!(selective_trust::TRUST_BOT_MESSAGES);
    assert!(selective_trust::TRUST_ADMIN_MESSAGES);
    assert!(!selective_trust::TRUST_VERIFIED_MESSAGES); // Should be false by default
    
    println!("Configuration constants tests passed");
    
    Ok(())
}