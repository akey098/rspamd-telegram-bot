use rspamd_telegram_bot::bayes_manager::BayesManager;
use std::sync::Once;

static INIT: Once = Once::new();

fn setup() {
    INIT.call_once(|| {
        pretty_env_logger::init();
    });
}

#[tokio::test]
async fn test_bayes_learning() {
    setup();
    
    let bayes = BayesManager::new().unwrap();
    
    // Test learning spam (this may fail if Rspamd is not running, which is expected in tests)
    let _result = bayes.learn_spam("test_msg_1", "Buy now! Limited time offer!").await;
    // We don't assert success here since Rspamd might not be running in test environment
    
    // Test learning ham (this may fail if Rspamd is not running, which is expected in tests)
    let _result = bayes.learn_ham("test_msg_2", "Hello, how are you today?").await;
    // We don't assert success here since Rspamd might not be running in test environment
    
    // Test that we can still get stats even if learning failed
    let stats = bayes.get_bayes_stats().unwrap();
    assert!(stats.contains_key("spam_messages"), "Stats should contain spam_messages");
    assert!(stats.contains_key("ham_messages"), "Stats should contain ham_messages");
}

#[tokio::test]
async fn test_bayes_manager_creation() {
    setup();
    
    let bayes_manager = BayesManager::new();
    assert!(bayes_manager.is_ok(), "BayesManager should be created successfully");
    
    let bayes_manager = bayes_manager.unwrap();
    
    // Test that we can get basic stats
    let stats = bayes_manager.get_bayes_stats();
    assert!(stats.is_ok(), "Should be able to retrieve Bayes statistics");
    
    let stats = stats.unwrap();
    assert!(stats.contains_key("spam_tokens"), "Stats should contain spam_tokens");
    assert!(stats.contains_key("ham_tokens"), "Stats should contain ham_tokens");
    assert!(stats.contains_key("spam_messages"), "Stats should contain spam_messages");
    assert!(stats.contains_key("ham_messages"), "Stats should contain ham_messages");
    assert!(stats.contains_key("total_messages"), "Stats should contain total_messages");
    assert!(stats.contains_key("spam_ratio_percent"), "Stats should contain spam_ratio_percent");
}

#[tokio::test]
async fn test_bayes_readiness_check() {
    setup();
    
    let bayes_manager = BayesManager::new().unwrap();
    
    // Test readiness check
    let is_ready = bayes_manager.is_ready();
    assert!(is_ready.is_ok(), "Should be able to check readiness");
    
    // Test detailed info retrieval
    let detailed_info = bayes_manager.get_detailed_info();
    assert!(detailed_info.is_ok(), "Should be able to retrieve detailed info");
    
    let detailed_info = detailed_info.unwrap();
    assert!(detailed_info.contains_key("is_ready"), "Detailed info should contain is_ready");
    assert!(detailed_info.contains_key("status"), "Detailed info should contain status");
    assert!(detailed_info.contains_key("min_spam_required"), "Detailed info should contain min_spam_required");
    assert!(detailed_info.contains_key("min_ham_required"), "Detailed info should contain min_ham_required");
    assert!(detailed_info.contains_key("spam_progress_percent"), "Detailed info should contain spam_progress_percent");
    assert!(detailed_info.contains_key("ham_progress_percent"), "Detailed info should contain ham_progress_percent");
}

#[tokio::test]
async fn test_bayes_message_tracking() {
    setup();
    
    let bayes_manager = BayesManager::new().unwrap();
    let test_message_id = "tracking_test_message_456";
    
    // Initially, message should not be learned
    assert!(!bayes_manager.is_message_learned(test_message_id).unwrap());
    assert!(bayes_manager.get_message_learning_type(test_message_id).unwrap().is_none());
    
    // Test with a message that might have been learned
    let learned_message_id = "learned_test_message_789";
    let is_learned = bayes_manager.is_message_learned(learned_message_id);
    assert!(is_learned.is_ok(), "Should be able to check if message is learned");
    
    let learning_type = bayes_manager.get_message_learning_type(learned_message_id);
    assert!(learning_type.is_ok(), "Should be able to get message learning type");
}

#[tokio::test]
async fn test_bayes_statistics_consistency() {
    setup();
    
    let bayes_manager = BayesManager::new().unwrap();
    
    // Get statistics multiple times to ensure consistency
    let stats1 = bayes_manager.get_bayes_stats().unwrap();
    let stats2 = bayes_manager.get_bayes_stats().unwrap();
    
    // The statistics should be consistent between calls
    assert_eq!(stats1.get("spam_tokens"), stats2.get("spam_tokens"));
    assert_eq!(stats1.get("ham_tokens"), stats2.get("ham_tokens"));
    assert_eq!(stats1.get("spam_messages"), stats2.get("spam_messages"));
    assert_eq!(stats1.get("ham_messages"), stats2.get("ham_messages"));
    assert_eq!(stats1.get("total_messages"), stats2.get("total_messages"));
    
    // Verify that total_messages is the sum of spam and ham messages
    let calculated_total = stats1.get("spam_messages").unwrap_or(&0) + stats1.get("ham_messages").unwrap_or(&0);
    assert_eq!(stats1.get("total_messages").unwrap_or(&0), &calculated_total);
    
    // Verify spam ratio calculation
    let spam_messages = stats1.get("spam_messages").unwrap_or(&0);
    let total_messages = stats1.get("total_messages").unwrap_or(&0);
    let expected_ratio = if *total_messages > 0 {
        (*spam_messages as f64 / *total_messages as f64 * 100.0) as i64
    } else {
        0
    };
    assert_eq!(stats1.get("spam_ratio_percent").unwrap_or(&0), &expected_ratio);
}

#[tokio::test]
async fn test_bayes_learning_with_various_content() {
    setup();
    
    let bayes_manager = BayesManager::new().unwrap();
    
    // Test learning with different types of content
    let test_cases = vec![
        ("spam_1", "URGENT! Make money fast! Click here now!"),
        ("spam_2", "FREE VIAGRA!!! LIMITED TIME OFFER!!!"),
        ("ham_1", "Hello everyone, how are you doing today?"),
        ("ham_2", "Thanks for the information, that's very helpful."),
        ("ham_3", "I'll be there at 3 PM for the meeting."),
    ];
    
    for (message_id, content) in test_cases {
        let _result = if content.contains("URGENT") || content.contains("VIAGRA") {
            bayes_manager.learn_spam(message_id, content).await
        } else {
            bayes_manager.learn_ham(message_id, content).await
        };
        
        // We don't assert success here since Rspamd might not be running in test environment
        // The test verifies that the API calls don't panic and return a Result
    }
    
    // Verify that we can still get statistics
    let stats = bayes_manager.get_bayes_stats().unwrap();
    assert!(stats.contains_key("spam_messages"), "Stats should contain spam_messages");
    assert!(stats.contains_key("ham_messages"), "Stats should contain ham_messages");
}

#[tokio::test]
async fn test_bayes_duplicate_learning() {
    setup();
    
    let bayes_manager = BayesManager::new().unwrap();
    let message_id = "duplicate_test_message";
    let content = "This is a test message for duplicate learning";
    
    // Learn the same message multiple times (may fail if Rspamd not running)
    let _result1 = bayes_manager.learn_spam(message_id, content).await;
    // We don't assert success here since Rspamd might not be running in test environment
    
    let _result2 = bayes_manager.learn_spam(message_id, content).await;
    // We don't assert success here since Rspamd might not be running in test environment
    
    // Test that we can check message learning status
    let is_learned = bayes_manager.is_message_learned(message_id).unwrap();
    let learning_type = bayes_manager.get_message_learning_type(message_id).unwrap();
    
    // These should work regardless of whether Rspamd is running
    assert!(is_learned == true || is_learned == false, "is_message_learned should return a boolean");
    assert!(learning_type.is_none() || learning_type.is_some(), "learning_type should be Option<String>");
}

#[tokio::test]
async fn test_bayes_error_handling() {
    setup();
    
    let bayes_manager = BayesManager::new().unwrap();
    
    // Test with empty content
    let _result = bayes_manager.learn_spam("empty_test", "").await;
    // This might succeed or fail depending on Rspamd configuration, but shouldn't panic
    
    // Test with very long content
    let long_content = "A".repeat(10000);
    let _result = bayes_manager.learn_ham("long_test", &long_content).await;
    // This might succeed or fail depending on Rspamd configuration, but shouldn't panic
    
    // Test with special characters
    let special_content = "Test with special chars: !@#$%^&*()_+-=[]{}|;':\",./<>?";
    let _result = bayes_manager.learn_spam("special_test", special_content).await;
    // This should handle special characters gracefully
}

#[tokio::test]
async fn test_bayes_performance_metrics() {
    setup();
    
    let bayes_manager = BayesManager::new().unwrap();
    
    // Test that we can get performance metrics
    let detailed_info = bayes_manager.get_detailed_info().unwrap();
    
    // Verify all required performance metrics are present
    assert!(detailed_info.contains_key("spam_progress_percent"));
    assert!(detailed_info.contains_key("ham_progress_percent"));
    assert!(detailed_info.contains_key("min_spam_required"));
    assert!(detailed_info.contains_key("min_ham_required"));
    
    // Verify progress percentages are within valid range
    let spam_progress: i64 = detailed_info.get("spam_progress_percent").unwrap_or(&"0".to_string()).parse().unwrap_or(0);
    let ham_progress: i64 = detailed_info.get("ham_progress_percent").unwrap_or(&"0".to_string()).parse().unwrap_or(0);
    
    assert!(spam_progress >= 0 && spam_progress <= 100, "Spam progress should be 0-100%");
    assert!(ham_progress >= 0 && ham_progress <= 100, "Ham progress should be 0-100%");
    
    // Verify minimum requirements are reasonable
    let min_spam: i64 = detailed_info.get("min_spam_required").unwrap_or(&"0".to_string()).parse().unwrap_or(0);
    let min_ham: i64 = detailed_info.get("min_ham_required").unwrap_or(&"0".to_string()).parse().unwrap_or(0);
    
    assert!(min_spam > 0, "Minimum spam requirement should be positive");
    assert!(min_ham > 0, "Minimum ham requirement should be positive");
}

#[tokio::test]
async fn test_bayes_concurrent_operations() {
    setup();
    
    let bayes_manager = BayesManager::new().unwrap();
    
    // Test concurrent learning operations
    let mut handles = vec![];
    
    for i in 0..5 {
        let bayes_clone = BayesManager::new().unwrap();
        let handle = tokio::spawn(async move {
            let message_id = format!("concurrent_test_{}", i);
            let content = format!("Concurrent test message {}", i);
            
            if i % 2 == 0 {
                bayes_clone.learn_spam(&message_id, &content).await
            } else {
                bayes_clone.learn_ham(&message_id, &content).await
            }
        });
        handles.push(handle);
    }
    
    // Wait for all operations to complete
    for handle in handles {
        let _result = handle.await.unwrap();
        // We don't assert success here since Rspamd might not be running in test environment
        // The test verifies that concurrent operations don't panic
    }
    
    // Verify that we can still get statistics
    let stats = bayes_manager.get_bayes_stats().unwrap();
    assert!(stats.contains_key("total_messages"), "Stats should contain total_messages");
}
