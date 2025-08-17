use rspamd_telegram_bot::admin_handlers::AdminCommand;
use rspamd_telegram_bot::bayes_manager::BayesManager;
use teloxide::utils::command::BotCommands;

#[test]
fn test_bayes_command_parsing() {
    // Test LearnSpam command parsing
    let learn_spam_cmd = AdminCommand::parse("/learnspam 12345", "test_bot").unwrap();
    match learn_spam_cmd {
        AdminCommand::LearnSpam { message_id } => {
            assert_eq!(message_id, "12345");
        }
        _ => panic!("Expected LearnSpam command"),
    }

    // Test LearnHam command parsing
    let learn_ham_cmd = AdminCommand::parse("/learnham 67890", "test_bot").unwrap();
    match learn_ham_cmd {
        AdminCommand::LearnHam { message_id } => {
            assert_eq!(message_id, "67890");
        }
        _ => panic!("Expected LearnHam command"),
    }

    // Test BayesStats command parsing
    let bayes_stats_cmd = AdminCommand::parse("/bayesstats", "test_bot").unwrap();
    match bayes_stats_cmd {
        AdminCommand::BayesStats => {
            // Command parsed successfully
        }
        _ => panic!("Expected BayesStats command"),
    }

    // Test BayesReset command parsing
    let bayes_reset_cmd = AdminCommand::parse("/bayesreset", "test_bot").unwrap();
    match bayes_reset_cmd {
        AdminCommand::BayesReset => {
            // Command parsed successfully
        }
        _ => panic!("Expected BayesReset command"),
    }
}

#[test]
fn test_bayes_manager_creation() {
    let bayes_manager = BayesManager::new();
    assert!(bayes_manager.is_ok());
}

#[test]
fn test_bayes_stats_retrieval() {
    let bayes_manager = BayesManager::new().unwrap();
    let stats = bayes_manager.get_bayes_stats();
    assert!(stats.is_ok());
    
    let stats = stats.unwrap();
    assert!(stats.contains_key("spam_tokens"));
    assert!(stats.contains_key("ham_tokens"));
    assert!(stats.contains_key("spam_messages"));
    assert!(stats.contains_key("ham_messages"));
    assert!(stats.contains_key("total_messages"));
}

#[test]
fn test_bayes_detailed_info() {
    let bayes_manager = BayesManager::new().unwrap();
    let info = bayes_manager.get_detailed_info();
    assert!(info.is_ok());
    
    let info = info.unwrap();
    assert!(info.contains_key("is_ready"));
    assert!(info.contains_key("status"));
    assert!(info.contains_key("min_spam_required"));
    assert!(info.contains_key("min_ham_required"));
    assert!(info.contains_key("spam_progress_percent"));
    assert!(info.contains_key("ham_progress_percent"));
}

#[test]
fn test_message_learning_tracking() {
    let bayes_manager = BayesManager::new().unwrap();
    let test_message_id = "test_message_123";
    
    // Initially, the message should not be learned
    let is_learned = bayes_manager.is_message_learned(test_message_id);
    assert!(is_learned.is_ok());
    assert!(!is_learned.unwrap());
    
    // Learning type should be None for unlearned message
    let learning_type = bayes_manager.get_message_learning_type(test_message_id);
    assert!(learning_type.is_ok());
    assert!(learning_type.unwrap().is_none());
}
