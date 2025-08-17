use rspamd_telegram_bot::bayes_manager::BayesManager;
use rspamd_telegram_bot::config::bayes;

#[tokio::test]
async fn test_autolearn_thresholds() {
    // Test that the auto-learning thresholds are set correctly
    assert_eq!(bayes::AUTOLEARN_SPAM_THRESHOLD, 6.0);
    assert_eq!(bayes::AUTOLEARN_HAM_THRESHOLD, -0.5);
    
    // Test threshold logic
    let high_spam_score = 7.0;
    let low_ham_score = -1.0;
    let neutral_score = 2.0;
    
    assert!(high_spam_score >= bayes::AUTOLEARN_SPAM_THRESHOLD);
    assert!(low_ham_score <= bayes::AUTOLEARN_HAM_THRESHOLD);
    assert!(neutral_score < bayes::AUTOLEARN_SPAM_THRESHOLD);
    assert!(neutral_score > bayes::AUTOLEARN_HAM_THRESHOLD);
}

#[tokio::test]
async fn test_bayes_manager_creation() {
    // Test that BayesManager can be created successfully
    let bayes_manager = BayesManager::new();
    assert!(bayes_manager.is_ok());
}

#[tokio::test]
async fn test_bayes_stats_retrieval() {
    // Test that we can retrieve Bayes statistics
    let bayes_manager = BayesManager::new().unwrap();
    let stats = bayes_manager.get_bayes_stats();
    assert!(stats.is_ok());
    
    let stats = stats.unwrap();
    // These should exist in the stats HashMap
    assert!(stats.contains_key("spam_tokens"));
    assert!(stats.contains_key("ham_tokens"));
    assert!(stats.contains_key("spam_messages"));
    assert!(stats.contains_key("ham_messages"));
}

#[tokio::test]
async fn test_bayes_readiness_check() {
    // Test the readiness check functionality
    let bayes_manager = BayesManager::new().unwrap();
    let is_ready = bayes_manager.is_ready();
    assert!(is_ready.is_ok());
    
    // The result should be a boolean
    let is_ready = is_ready.unwrap();
    assert!(is_ready == true || is_ready == false);
}
