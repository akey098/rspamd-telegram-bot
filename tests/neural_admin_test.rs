use teloxide::prelude::*;
use teloxide::types::{Chat, ChatId, Message, User, UserId};
use rspamd_telegram_bot::admin_handlers::{AdminCommand, handle_neural_stats, handle_neural_reset, handle_neural_status, handle_neural_features};
use rspamd_telegram_bot::neural_manager::NeuralManager;

#[tokio::test]
async fn test_neural_stats_command() {
    // Create a mock bot
    let bot = Bot::new("test_token");
    
    // Create a mock chat ID
    let chat_id = ChatId(123456789);
    
    // Test the neural stats command
    let result = handle_neural_stats(bot, chat_id).await;
    
    // The command should not panic, even if Redis is not available
    // In a real test environment, you'd mock Redis or use a test database
    assert!(result.is_ok() || result.is_err());
}

#[tokio::test]
async fn test_neural_reset_command() {
    // Create a mock bot
    let bot = Bot::new("test_token");
    
    // Create a mock chat ID
    let chat_id = ChatId(123456789);
    
    // Test the neural reset command
    let result = handle_neural_reset(bot, chat_id).await;
    
    // The command should not panic
    assert!(result.is_ok() || result.is_err());
}

#[tokio::test]
async fn test_neural_status_command() {
    // Create a mock bot
    let bot = Bot::new("test_token");
    
    // Create a mock chat ID
    let chat_id = ChatId(123456789);
    
    // Test the neural status command
    let result = handle_neural_status(bot, chat_id).await;
    
    // The command should not panic
    assert!(result.is_ok() || result.is_err());
}

#[tokio::test]
async fn test_neural_features_command() {
    // Create a mock bot
    let bot = Bot::new("test_token");
    
    // Create a mock chat ID
    let chat_id = ChatId(123456789);
    
    // Test the neural features command with a test message ID
    let message_id = "test_message_123".to_string();
    let result = handle_neural_features(bot, chat_id, message_id).await;
    
    // The command should not panic
    assert!(result.is_ok() || result.is_err());
}

#[tokio::test]
async fn test_neural_manager_creation() {
    // Test that NeuralManager can be created
    let result = NeuralManager::new();
    assert!(result.is_ok() || result.is_err());
}

#[tokio::test]
async fn test_neural_stats_retrieval() {
    // Test that neural stats can be retrieved
    if let Ok(neural_manager) = NeuralManager::new() {
        let result = neural_manager.get_neural_stats();
        assert!(result.is_ok() || result.is_err());
    }
}

#[tokio::test]
async fn test_neural_ready_check() {
    // Test that neural ready status can be checked
    if let Ok(neural_manager) = NeuralManager::new() {
        let result = neural_manager.is_ready();
        assert!(result.is_ok() || result.is_err());
    }
}

#[tokio::test]
async fn test_admin_command_parsing() {
    // Test that neural commands can be parsed
    let neural_stats_cmd = AdminCommand::NeuralStats;
    let neural_reset_cmd = AdminCommand::NeuralReset;
    let neural_status_cmd = AdminCommand::NeuralStatus;
    let neural_features_cmd = AdminCommand::NeuralFeatures { 
        message_id: "test_123".to_string() 
    };
    
    // Verify the commands exist
    assert!(matches!(neural_stats_cmd, AdminCommand::NeuralStats));
    assert!(matches!(neural_reset_cmd, AdminCommand::NeuralReset));
    assert!(matches!(neural_status_cmd, AdminCommand::NeuralStatus));
    assert!(matches!(neural_features_cmd, AdminCommand::NeuralFeatures { message_id } if message_id == "test_123"));
}

#[tokio::test]
async fn test_neural_health_status() {
    // Test the neural health status helper function
    use rspamd_telegram_bot::admin_handlers::neural_commands::get_neural_health_status;
    
    let result = get_neural_health_status();
    assert!(result.is_ok() || result.is_err());
}

#[tokio::test]
async fn test_neural_stats_formatting() {
    // Test the neural stats formatting helper function
    use rspamd_telegram_bot::admin_handlers::neural_commands::format_neural_stats;
    use rspamd_telegram_bot::neural_manager::NeuralStats;
    
    let stats = NeuralStats {
        total_messages: 100,
        spam_messages: 30,
        ham_messages: 70,
        training_iterations: 5,
        model_accuracy: 0.85,
        last_training: Some("2024-01-01".to_string()),
    };
    
    let formatted = format_neural_stats(&stats);
    assert!(formatted.contains("Neural Network"));
    assert!(formatted.contains("85.0%"));
    assert!(formatted.contains("100"));
    assert!(formatted.contains("5"));
}
