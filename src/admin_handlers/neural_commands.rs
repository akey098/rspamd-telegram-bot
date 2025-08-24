use teloxide::prelude::*;
use teloxide::types::ParseMode;
use crate::neural_manager::NeuralManager;
use crate::config::neural;
use redis::Commands;

use anyhow::Result;

/// Handles the /neuralstats command to show neural network statistics
pub async fn handle_neural_stats(bot: Bot, chat_id: ChatId) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let neural_manager = NeuralManager::new()?;
    
    match neural_manager.get_neural_stats() {
        Ok(stats) => {
            let status = if neural_manager.is_ready()? { "✅ Ready" } else { "🔄 Training" };
            let accuracy_percent = stats.model_accuracy * 100.0;
            
            let response = format!(
                "🤖 *Neural Network Statistics*\n\n\
                **Status:** {}\n\
                **Total Messages:** {}\n\
                **Spam Messages:** {}\n\
                **Ham Messages:** {}\n\
                **Training Iterations:** {}\n\
                **Model Accuracy:** {:.1}%\n\
                **Last Training:** {}\n\n\
                **Configuration:**\n\
                • Min Samples Required: {}\n\
                • Confidence Threshold: {:.1}\n\
                • Spam Threshold: {:.1}\n\
                • Ham Threshold: {:.1}",
                status,
                stats.total_messages,
                stats.spam_messages,
                stats.ham_messages,
                stats.training_iterations,
                accuracy_percent,
                stats.last_training.unwrap_or_else(|| "Never".to_string()),
                neural::MIN_SAMPLES_REQUIRED,
                neural::CONFIDENCE_THRESHOLD,
                neural::SPAM_THRESHOLD,
                neural::HAM_THRESHOLD
            );
            
            bot.send_message(chat_id, response)
                .parse_mode(ParseMode::Markdown)
                .await?;
        }
        Err(e) => {
            bot.send_message(chat_id, format!("❌ Error getting neural stats: {}", e)).await?;
        }
    }
    
    Ok(())
}

/// Handles the /neuralreset command to reset neural network model and training data
pub async fn handle_neural_reset(bot: Bot, chat_id: ChatId) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _neural_manager = NeuralManager::new()?;
    let redis_client = redis::Client::open("redis://127.0.0.1/")?;
    let mut conn = redis_client.get_connection()?;
    
    // Reset neural network statistics
    let _: () = conn.del(neural::NEURAL_STATS_KEY)?;
    let _: () = conn.del(neural::NEURAL_MODEL_KEY)?;
    let _: () = conn.del(neural::NEURAL_FEATURES_KEY)?;
    
    // Initialize with default values
    let _: () = conn.hset_multiple(neural::NEURAL_STATS_KEY, &[
        ("total_messages", "0"),
        ("spam_messages", "0"),
        ("ham_messages", "0"),
        ("training_iterations", "0"),
        ("model_accuracy", "0.0"),
        ("last_training", "Never"),
    ])?;
    
    bot.send_message(
        chat_id,
        "🔄 *Neural Network Reset Complete*\n\n\
        All neural network data has been cleared:\n\
        • Training statistics reset\n\
        • Model data cleared\n\
        • Feature cache cleared\n\n\
        The neural network will start fresh training when sufficient data is available.",
    )
    .parse_mode(ParseMode::Markdown)
    .await?;
    
    Ok(())
}

/// Handles the /neuralstatus command to show detailed neural network training status
pub async fn handle_neural_status(bot: Bot, chat_id: ChatId) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let neural_manager = NeuralManager::new()?;
    
    match neural_manager.get_neural_stats() {
        Ok(stats) => {
            let is_ready = neural_manager.is_ready()?;
            let accuracy_percent = stats.model_accuracy * 100.0;
            
            // Calculate training progress
            let progress_percent = if neural::MIN_SAMPLES_REQUIRED > 0 {
                (stats.total_messages as f64 / neural::MIN_SAMPLES_REQUIRED as f64 * 100.0).min(100.0)
            } else {
                0.0
            };
            
            let status_emoji = if is_ready { "✅" } else { "🔄" };
            let status_text = if is_ready { "Ready for Classification" } else { "Training in Progress" };
            
            let response = format!(
                "🤖 *Neural Network Training Status*\n\n\
                **Overall Status:** {} {}\n\n\
                **Training Progress:**\n\
                • Progress: {:.1}% ({}/{})\n\
                • Training Iterations: {}\n\
                • Current Accuracy: {:.1}%\n\n\
                **Data Distribution:**\n\
                • Spam Samples: {} ({:.1}%)\n\
                • Ham Samples: {} ({:.1}%)\n\
                • Total Samples: {}\n\n\
                **Model Information:**\n\
                • Last Training: {}\n\
                • Confidence Threshold: {:.1}\n\
                • Ready for Classification: {}",
                status_emoji,
                status_text,
                progress_percent,
                stats.total_messages,
                neural::MIN_SAMPLES_REQUIRED,
                stats.training_iterations,
                accuracy_percent,
                stats.spam_messages,
                if stats.total_messages > 0 { (stats.spam_messages as f64 / stats.total_messages as f64) * 100.0 } else { 0.0 },
                stats.ham_messages,
                if stats.total_messages > 0 { (stats.ham_messages as f64 / stats.total_messages as f64) * 100.0 } else { 0.0 },
                stats.total_messages,
                stats.last_training.unwrap_or_else(|| "Never".to_string()),
                neural::CONFIDENCE_THRESHOLD,
                if is_ready { "Yes" } else { "No" }
            );
            
            bot.send_message(chat_id, response)
                .parse_mode(ParseMode::Markdown)
                .await?;
        }
        Err(e) => {
            bot.send_message(chat_id, format!("❌ Error getting neural status: {}", e)).await?;
        }
    }
    
    Ok(())
}

/// Handles the /neuralfeatures command to show neural network feature analysis for a specific message
pub async fn handle_neural_features(bot: Bot, chat_id: ChatId, message_id: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _neural_manager = NeuralManager::new()?;
    let redis_client = redis::Client::open("redis://127.0.0.1/")?;
    let mut conn = redis_client.get_connection()?;
    
    // Try to get the message content from Redis
    let message_key = format!("message:{}", message_id);
    let message_content: Option<String> = conn.get(&message_key).ok();
    
    if let Some(content) = message_content {
        // For now, we'll show a placeholder since we don't have the actual Rspamd scan result
        // In a real implementation, you'd store the scan results and retrieve them here
        let response = format!(
            "🔍 *Neural Network Feature Analysis*\n\n\
            **Message ID:** `{}`\n\
            **Content Length:** {} characters\n\
            **Word Count:** {} words\n\n\
            **Feature Analysis:**\n\
            • Text Features: Available\n\
            • Symbol Features: Available\n\
            • Metadata Features: Available\n\n\
            **Note:** Detailed feature analysis requires the message to be processed by Rspamd with neural network module enabled.\n\n\
            To get detailed features, ensure:\n\
            1. Neural network module is enabled in Rspamd\n\
            2. Message has been scanned by Rspamd\n\
            3. Neural network has sufficient training data",
            message_id,
            content.len(),
            content.split_whitespace().count()
        );
        
        bot.send_message(chat_id, response)
            .parse_mode(ParseMode::Markdown)
            .await?;
    } else {
        bot.send_message(
            chat_id,
            format!(
                "❌ *Message Not Found*\n\n\
                Could not find message with ID: `{}`\n\n\
                Make sure the message exists and has been processed by the bot.",
                message_id
            )
        )
        .parse_mode(ParseMode::Markdown)
        .await?;
    }
    
    Ok(())
}

/// Helper function to format neural network statistics for display
pub fn format_neural_stats(stats: &crate::neural_manager::NeuralStats) -> String {
    let accuracy_percent = stats.model_accuracy * 100.0;
    let status = if stats.total_messages >= neural::MIN_SAMPLES_REQUIRED && stats.training_iterations > 0 {
        "✅ Ready"
    } else {
        "🔄 Training"
    };
    
    format!(
        "🤖 Neural Network: {} | Accuracy: {:.1}% | Messages: {} | Iterations: {}",
        status,
        accuracy_percent,
        stats.total_messages,
        stats.training_iterations
    )
}

/// Helper function to get neural network health status
pub fn get_neural_health_status() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let neural_manager = NeuralManager::new()?;
    let stats = neural_manager.get_neural_stats()?;
    let is_ready = neural_manager.is_ready()?;
    
    let health_status = if is_ready {
        "healthy"
    } else if stats.total_messages > 0 {
        "training"
    } else {
        "inactive"
    };
    
    Ok(format!(
        "neural_network:{}:{}:{:.1}",
        health_status,
        stats.total_messages,
        stats.model_accuracy * 100.0
    ))
}
