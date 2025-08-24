use rspamd_telegram_bot::neural_manager::NeuralManager;

#[tokio::test]
async fn test_neural_manager_basic() {
    let manager = NeuralManager::new();
    assert!(manager.is_ok(), "NeuralManager should be created successfully");
}

#[tokio::test]
async fn test_neural_stats_basic() {
    let manager = NeuralManager::new().unwrap();
    let stats = manager.get_neural_stats();
    assert!(stats.is_ok(), "Should be able to retrieve neural statistics");
}
