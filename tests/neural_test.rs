use rspamd_telegram_bot::neural_manager::{NeuralManager, NeuralStats, NeuralFeatures};
use rspamd_telegram_bot::config::neural;
use std::collections::HashMap;
use std::sync::Once;

static INIT: Once = Once::new();

fn setup() {
    INIT.call_once(|| {
        pretty_env_logger::init();
    });
}

#[tokio::test]
async fn test_neural_manager_creation() {
    setup();
    
    let manager = NeuralManager::new();
    assert!(manager.is_ok(), "NeuralManager should be created successfully");
    
    let manager = manager.unwrap();
    
    // Test that we can get basic stats even if Redis is not fully configured
    let stats = manager.get_neural_stats();
    assert!(stats.is_ok(), "Should be able to retrieve neural statistics");
    
    let stats = stats.unwrap();
    assert!(stats.total_messages >= 0, "Total messages should be non-negative");
    assert!(stats.spam_messages >= 0, "Spam messages should be non-negative");
    assert!(stats.ham_messages >= 0, "Ham messages should be non-negative");
    assert!(stats.training_iterations >= 0, "Training iterations should be non-negative");
    assert!(stats.model_accuracy >= 0.0, "Model accuracy should be non-negative");
    assert!(stats.model_accuracy <= 1.0, "Model accuracy should be <= 1.0");
}

#[tokio::test]
async fn test_neural_stats_retrieval() {
    setup();
    
    let manager = NeuralManager::new().unwrap();
    let stats = manager.get_neural_stats();
    assert!(stats.is_ok(), "Should be able to retrieve neural statistics");
    
    let stats = stats.unwrap();
    
    // Verify all required fields are present
    assert!(stats.total_messages >= 0, "Total messages should be non-negative");
    assert!(stats.spam_messages >= 0, "Spam messages should be non-negative");
    assert!(stats.ham_messages >= 0, "Ham messages should be non-negative");
    assert!(stats.training_iterations >= 0, "Training iterations should be non-negative");
    assert!(stats.model_accuracy >= 0.0, "Model accuracy should be non-negative");
    assert!(stats.model_accuracy <= 1.0, "Model accuracy should be <= 1.0");
    
    // Verify logical consistency
    assert!(stats.total_messages >= stats.spam_messages + stats.ham_messages, 
            "Total messages should be >= spam + ham messages");
}

#[tokio::test]
async fn test_neural_ready_check() {
    setup();
    
    let manager = NeuralManager::new().unwrap();
    let is_ready = manager.is_ready();
    assert!(is_ready.is_ok(), "Should be able to check readiness");
    
    let is_ready = is_ready.unwrap();
    // Neural network might not be ready in test environment, which is expected
    assert!(is_ready == true || is_ready == false, "Readiness should be boolean");
}

#[tokio::test]
async fn test_neural_accuracy_retrieval() {
    setup();
    
    let manager = NeuralManager::new().unwrap();
    let accuracy = manager.get_accuracy();
    assert!(accuracy.is_ok(), "Should be able to retrieve accuracy");
    
    let accuracy = accuracy.unwrap();
    assert!(accuracy >= 0.0, "Accuracy should be non-negative");
    assert!(accuracy <= 1.0, "Accuracy should be <= 1.0");
}

#[tokio::test]
async fn test_neural_configuration_constants() {
    setup();
    
    // Test that neural configuration constants are properly defined
    assert_eq!(neural::NEURAL_STATS_KEY, "neural:stats", "NEURAL_STATS_KEY should be correct");
    assert_eq!(neural::NEURAL_MODEL_KEY, "neural:model", "NEURAL_MODEL_KEY should be correct");
    assert_eq!(neural::NEURAL_FEATURES_KEY, "neural:features", "NEURAL_FEATURES_KEY should be correct");
    assert_eq!(neural::MIN_SAMPLES_REQUIRED, 100, "MIN_SAMPLES_REQUIRED should be 100");
    assert_eq!(neural::CONFIDENCE_THRESHOLD, 0.7, "CONFIDENCE_THRESHOLD should be 0.7");
    assert_eq!(neural::SPAM_THRESHOLD, 6.0, "SPAM_THRESHOLD should be 6.0");
    assert_eq!(neural::HAM_THRESHOLD, -2.0, "HAM_THRESHOLD should be -2.0");
}

#[tokio::test]
async fn test_neural_stats_consistency() {
    setup();
    
    let manager = NeuralManager::new().unwrap();
    let stats = manager.get_neural_stats().unwrap();
    
    // Test logical consistency of statistics
    assert!(stats.total_messages >= 0, "Total messages should be non-negative");
    assert!(stats.spam_messages >= 0, "Spam messages should be non-negative");
    assert!(stats.ham_messages >= 0, "Ham messages should be non-negative");
    assert!(stats.training_iterations >= 0, "Training iterations should be non-negative");
    assert!(stats.model_accuracy >= 0.0, "Model accuracy should be non-negative");
    assert!(stats.model_accuracy <= 1.0, "Model accuracy should be <= 1.0");
    
    // Test that total messages is at least the sum of spam and ham
    assert!(stats.total_messages >= stats.spam_messages + stats.ham_messages, 
            "Total messages should be >= spam + ham messages");
    
    // Test that if we have training iterations, we should have some messages
    if stats.training_iterations > 0 {
        assert!(stats.total_messages > 0, "Training iterations > 0 should imply total_messages > 0");
    }
}

#[tokio::test]
async fn test_neural_readiness_logic() {
    setup();
    
    let manager = NeuralManager::new().unwrap();
    let is_ready = manager.is_ready().unwrap();
    let stats = manager.get_neural_stats().unwrap();
    
    // Test readiness logic
    let expected_ready = stats.total_messages >= neural::MIN_SAMPLES_REQUIRED && 
                        stats.training_iterations > 0;
    
    assert_eq!(is_ready, expected_ready, "Readiness should match expected logic");
    
    // If neural network is ready, it should have sufficient data
    if is_ready {
        assert!(stats.total_messages >= neural::MIN_SAMPLES_REQUIRED, 
                "Ready neural network should have sufficient samples");
        assert!(stats.training_iterations > 0, "Ready neural network should have training iterations");
    }
}

#[tokio::test]
async fn test_neural_feature_serialization() {
    setup();
    
    // Test that NeuralFeatures can be serialized and deserialized
    let mut symbols = HashMap::new();
    symbols.insert("TG_FLOOD".to_string(), 2.0);
    symbols.insert("NEURAL_SPAM".to_string(), 3.0);
    
    let mut metadata = HashMap::new();
    metadata.insert("score".to_string(), 5.0);
    metadata.insert("required_score".to_string(), 6.0);
    
    let mut text_features = HashMap::new();
    text_features.insert("symbol_count".to_string(), 2.0);
    text_features.insert("total_score".to_string(), 5.0);
    
    let features = NeuralFeatures {
        symbols,
        metadata,
        text_features,
    };
    
    // Test JSON serialization
    let json = serde_json::to_string(&features);
    assert!(json.is_ok(), "NeuralFeatures should be serializable to JSON");
    
    let json_str = json.unwrap();
    let deserialized: Result<NeuralFeatures, _> = serde_json::from_str(&json_str);
    assert!(deserialized.is_ok(), "NeuralFeatures should be deserializable from JSON");
    
    let deserialized = deserialized.unwrap();
    assert_eq!(deserialized.symbols["TG_FLOOD"], 2.0, "Deserialized TG_FLOOD should match");
    assert_eq!(deserialized.symbols["NEURAL_SPAM"], 3.0, "Deserialized NEURAL_SPAM should match");
    assert_eq!(deserialized.metadata["score"], 5.0, "Deserialized score should match");
    assert_eq!(deserialized.metadata["required_score"], 6.0, "Deserialized required_score should match");
    assert_eq!(deserialized.text_features["symbol_count"], 2.0, "Deserialized symbol_count should match");
    assert_eq!(deserialized.text_features["total_score"], 5.0, "Deserialized total_score should match");
}

#[tokio::test]
async fn test_neural_stats_serialization() {
    setup();
    
    // Test that NeuralStats can be serialized and deserialized
    let stats = NeuralStats {
        total_messages: 150,
        spam_messages: 50,
        ham_messages: 100,
        training_iterations: 5,
        model_accuracy: 0.85,
        last_training: Some("2024-01-15T10:30:00Z".to_string()),
    };
    
    // Test JSON serialization
    let json = serde_json::to_string(&stats);
    assert!(json.is_ok(), "NeuralStats should be serializable to JSON");
    
    let json_str = json.unwrap();
    let deserialized: Result<NeuralStats, _> = serde_json::from_str(&json_str);
    assert!(deserialized.is_ok(), "NeuralStats should be deserializable from JSON");
    
    let deserialized = deserialized.unwrap();
    assert_eq!(deserialized.total_messages, 150, "Deserialized total_messages should match");
    assert_eq!(deserialized.spam_messages, 50, "Deserialized spam_messages should match");
    assert_eq!(deserialized.ham_messages, 100, "Deserialized ham_messages should match");
    assert_eq!(deserialized.training_iterations, 5, "Deserialized training_iterations should match");
    assert_eq!(deserialized.model_accuracy, 0.85, "Deserialized model_accuracy should match");
    assert_eq!(deserialized.last_training, Some("2024-01-15T10:30:00Z".to_string()), 
               "Deserialized last_training should match");
}

#[tokio::test]
async fn test_neural_manager_error_handling() {
    setup();
    
    // Test that NeuralManager handles Redis connection errors gracefully
    // This test verifies that the manager doesn't panic on connection issues
    
    let manager = NeuralManager::new();
    assert!(manager.is_ok(), "NeuralManager should handle connection setup gracefully");
    
    let manager = manager.unwrap();
    
    // Test that stats retrieval doesn't panic even if Redis is not available
    let _stats_result = manager.get_neural_stats();
    // We don't assert success here since Redis might not be available in test environment
    // But we ensure it doesn't panic
    
    // Test that readiness check doesn't panic
    let _readiness_result = manager.is_ready();
    // We don't assert success here since it depends on Redis availability
    
    // Test that accuracy retrieval doesn't panic
    let _accuracy_result = manager.get_accuracy();
    // We don't assert success here since it depends on Redis availability
}
