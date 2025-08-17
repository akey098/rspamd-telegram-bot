use rspamd_telegram_bot::bayes_manager::BayesManager;
use std::sync::Once;

static INIT: Once = Once::new();

fn setup() {
    INIT.call_once(|| {
        pretty_env_logger::init();
    });
}

#[tokio::test]
async fn test_bayes_manager_integration() {
    setup();
    
    // Test BayesManager creation
    let bayes_manager = BayesManager::new();
    assert!(bayes_manager.is_ok(), "BayesManager should be created successfully");
    
    let bayes_manager = bayes_manager.unwrap();
    
    // Test statistics retrieval
    let stats = bayes_manager.get_bayes_stats();
    assert!(stats.is_ok(), "Should be able to retrieve Bayes statistics");
    
    let stats = stats.unwrap();
    assert!(stats.contains_key("spam_tokens"), "Stats should contain spam_tokens");
    assert!(stats.contains_key("ham_tokens"), "Stats should contain ham_tokens");
    assert!(stats.contains_key("spam_messages"), "Stats should contain spam_messages");
    assert!(stats.contains_key("ham_messages"), "Stats should contain ham_messages");
    assert!(stats.contains_key("total_messages"), "Stats should contain total_messages");
    assert!(stats.contains_key("spam_ratio_percent"), "Stats should contain spam_ratio_percent");
    
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
    
    // Test message learning tracking
    let test_message_id = "integration_test_message_123";
    let is_learned = bayes_manager.is_message_learned(test_message_id);
    assert!(is_learned.is_ok(), "Should be able to check if message is learned");
    
    let learning_type = bayes_manager.get_message_learning_type(test_message_id);
    assert!(learning_type.is_ok(), "Should be able to get message learning type");
    assert!(learning_type.unwrap().is_none(), "Unlearned message should have no learning type");
}

#[tokio::test]
async fn test_bayes_manager_message_tracking() {
    setup();
    
    let bayes_manager = BayesManager::new().unwrap();
    let test_message_id = "tracking_test_message_456";
    
    // Initially, message should not be learned
    assert!(!bayes_manager.is_message_learned(test_message_id).unwrap());
    assert!(bayes_manager.get_message_learning_type(test_message_id).unwrap().is_none());
    
    // Note: We don't actually call learn_spam/learn_ham in tests because it requires
    // a running Rspamd instance, but we can test the tracking methods work correctly
    // for unlearned messages
}

#[tokio::test]
async fn test_bayes_manager_statistics_consistency() {
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
        (spam_messages * 100) / total_messages
    } else {
        0
    };
    assert_eq!(stats1.get("spam_ratio_percent").unwrap_or(&0), &expected_ratio);
}

#[tokio::test]
async fn test_bayes_manager_readiness_logic() {
    setup();
    
    let bayes_manager = BayesManager::new().unwrap();
    
    // Test readiness check
    let is_ready = bayes_manager.is_ready().unwrap();
    
    // Get detailed info to verify readiness logic
    let detailed_info = bayes_manager.get_detailed_info().unwrap();
    let detailed_is_ready = detailed_info.get("is_ready").unwrap() == "true";
    
    // The readiness check should be consistent between methods
    assert_eq!(is_ready, detailed_is_ready);
    
    // Verify status field matches readiness
    let status = detailed_info.get("status").unwrap();
    if is_ready {
        assert_eq!(status, "Ready");
    } else {
        assert_eq!(status, "Training");
    }
    
    // Verify progress calculations
    let spam_progress: i64 = detailed_info.get("spam_progress_percent").unwrap().parse().unwrap();
    let ham_progress: i64 = detailed_info.get("ham_progress_percent").unwrap().parse().unwrap();
    
    assert!(spam_progress >= 0 && spam_progress <= 100, "Spam progress should be between 0 and 100");
    assert!(ham_progress >= 0 && ham_progress <= 100, "Ham progress should be between 0 and 100");
}
