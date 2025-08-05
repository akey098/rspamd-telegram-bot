use rspamd_telegram_bot::trust_manager::{TrustManager, TrustedMessageType, TrustedMessageMetadata};
use teloxide::types::{ChatId, MessageId, UserId};
use chrono::Utc;

#[tokio::test]
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
async fn test_trusted_message_type_scoring() {
    // Test score reductions
    assert_eq!(TrustedMessageType::Bot.score_reduction(), -3.0);
    assert_eq!(TrustedMessageType::Admin.score_reduction(), -2.0);
    assert_eq!(TrustedMessageType::Verified.score_reduction(), -1.0);
}

#[tokio::test]
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
    let expected_key = "tg:trusted:123";
    assert_eq!(metadata.redis_key(), expected_key);
    
    let expected_metadata_key = "tg:trusted:123:metadata123";
    assert_eq!(metadata.metadata_key(), expected_metadata_key);
}

#[tokio::test]
async fn test_trust_manager_creation() {
    // Test successful creation
    let trust_manager = TrustManager::new("redis://127.0.0.1/");
    assert!(trust_manager.is_ok());
    
    // Test failed creation with invalid URL
    let trust_manager = TrustManager::new("invalid://url");
    assert!(trust_manager.is_err());
}

#[tokio::test]
async fn test_trusted_message_lifecycle() {
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
async fn test_reply_tracking() {
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
async fn test_stats_collection() {
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
async fn test_different_trust_types() {
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
async fn test_cleanup_functionality() {
    let trust_manager = TrustManager::new("redis://127.0.0.1/").unwrap();
    
    // Test cleanup (should not error)
    let result = trust_manager.cleanup_expired().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_error_handling() {
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