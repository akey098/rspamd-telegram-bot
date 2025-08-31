use redis::Commands;
use reqwest::Client;
use std::collections::HashMap;
use anyhow::Result;
use crate::config::{rspamd, bayes, neural};
use crate::neural_manager::NeuralManager;
use chrono::Utc;
use serde::{Deserialize, Serialize};

/// Text features extracted for neural network training.
#[derive(Debug, Serialize, Deserialize)]
pub struct TextFeatures {
    pub word_count: usize,
    pub link_count: usize,
    pub emoji_count: usize,
    pub caps_ratio: f64,
}

/// Combined statistics for both Bayesian and Neural Network classifiers.
#[derive(Debug, Serialize, Deserialize)]
pub struct CombinedStats {
    pub bayes: HashMap<String, i64>,
    pub neural: crate::neural_manager::NeuralStats,
}

/// Readiness status for both classifiers.
#[derive(Debug, Serialize, Deserialize)]
pub struct ClassifierReadiness {
    pub bayes_ready: bool,
    pub neural_ready: bool,
    pub both_ready: bool,
}

/// Detailed information about both classifiers.
#[derive(Debug, Serialize, Deserialize)]
pub struct DetailedClassifierInfo {
    pub bayes: HashMap<String, String>,
    pub neural: HashMap<String, String>,
}

/// Manages Bayesian learning operations for the Rspamd Telegram bot.
/// 
/// This struct provides functionality to:
/// - Learn messages as spam or ham via Rspamd HTTP API
/// - Track learning statistics in Redis
/// - Monitor classifier readiness
/// - Manage learned message tracking
/// - Integrate with neural network training
pub struct BayesManager {
    redis_client: redis::Client,
    rspamd_client: Client,
    rspamd_url: String,
    rspamd_password: String,
}

impl BayesManager {
    /// Creates a new BayesManager instance.
    /// 
    /// # Returns
    /// 
    /// A `Result<Self>` containing the BayesManager or an error if initialization fails.
    pub fn new() -> Result<Self> {
        let redis_client = redis::Client::open("redis://127.0.0.1/")?;
        let rspamd_client = Client::new();
        
        Ok(Self {
            redis_client,
            rspamd_client,
            rspamd_url: rspamd::CONTROLLER_URL.to_string(),
            rspamd_password: rspamd::PASSWORD.to_string(),
        })
    }
    
    /// Learns a message as spam via Rspamd HTTP API and triggers neural network training.
    /// 
    /// # Arguments
    /// 
    /// * `message_id` - The unique identifier for the message
    /// * `content` - The message content to learn as spam
    /// 
    /// # Returns
    /// 
    /// A `Result<()>` indicating success or failure of the learning operation.
    pub async fn learn_spam(&self, message_id: &str, content: &str) -> Result<()> {
        // Format content as a proper email message with headers
        let email_content = format!(
            "Message-ID: <{}@telegram.bot>\r\n\
             From: telegram-bot@local\r\n\
             To: rspamd@local\r\n\
             Subject: Telegram message {}\r\n\
             Content-Type: text/plain; charset=utf-8\r\n\
             \r\n\
             {}",
            message_id, message_id, content
        );
        
        let url = format!("{}/learnspam", self.rspamd_url);
        
        let response = self.rspamd_client
            .post(&url)
            .header("Password", &self.rspamd_password)
            .header("Content-Type", "message/rfc822")
            .body(email_content)
            .send()
            .await?;
            
        let status = response.status();
        if status.is_success() {
            // Store learning record in Redis
            let mut conn = self.redis_client.get_connection()?;
            let key = format!("{}spam:{}", bayes::BAYES_LEARNED_PREFIX, message_id);
            let _: () = conn.set_ex(&key, "1", bayes::LEARNED_EXPIRY)?;
            
            // Increment spam message counter
            let _: i64 = conn.incr(bayes::BAYES_SPAM_MESSAGES_KEY, 1)?;
            
            // Integrate with neural network training
            self.update_neural_training_stats("spam", message_id, content).await?;
            
            log::info!("Successfully learned message {} as spam", message_id);
            Ok(())
        } else {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            
            // Handle specific Rspamd error cases
            if error_text.contains("already learned") {
                log::warn!("Message {} already learned as spam, ignoring duplicate", message_id);
                return Ok(()); // Treat as success since the message is already learned
            }
            
            log::error!("Failed to learn as spam: {} - {}", status, error_text);
            Err(anyhow::anyhow!("Failed to learn as spam: {} - {}", status, error_text))
        }
    }
    
    /// Learns a message as ham via Rspamd HTTP API and triggers neural network training.
    /// 
    /// # Arguments
    /// 
    /// * `message_id` - The unique identifier for the message
    /// * `content` - The message content to learn as ham
    /// 
    /// # Returns
    /// 
    /// A `Result<()>` indicating success or failure of the learning operation.
    pub async fn learn_ham(&self, message_id: &str, content: &str) -> Result<()> {
        // Format content as a proper email message with headers
        let email_content = format!(
            "Message-ID: <{}@telegram.bot>\r\n\
             From: telegram-bot@local\r\n\
             To: rspamd@local\r\n\
             Subject: Telegram message {}\r\n\
             Content-Type: text/plain; charset=utf-8\r\n\
             \r\n\
             {}",
            message_id, message_id, content
        );
        
        let url = format!("{}/learnham", self.rspamd_url);
        
        let response = self.rspamd_client
            .post(&url)
            .header("Password", &self.rspamd_password)
            .header("Content-Type", "message/rfc822")
            .body(email_content)
            .send()
            .await?;
            
        let status = response.status();
        if status.is_success() {
            // Store learning record in Redis
            let mut conn = self.redis_client.get_connection()?;
            let key = format!("{}ham:{}", bayes::BAYES_LEARNED_PREFIX, message_id);
            let _: () = conn.set_ex(&key, "1", bayes::LEARNED_EXPIRY)?;
            
            // Increment ham message counter
            let _: i64 = conn.incr(bayes::BAYES_HAM_MESSAGES_KEY, 1)?;
            
            // Integrate with neural network training
            self.update_neural_training_stats("ham", message_id, content).await?;
            
            log::info!("Successfully learned message {} as ham", message_id);
            Ok(())
        } else {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            
            // Handle specific Rspamd error cases
            if error_text.contains("already learned") {
                log::warn!("Message {} already learned as ham, ignoring duplicate", message_id);
                return Ok(()); // Treat as success since the message is already learned
            }
            
            log::error!("Failed to learn as ham: {} - {}", status, error_text);
            Err(anyhow::anyhow!("Failed to learn as ham: {} - {}", status, error_text))
        }
    }
    
    /// Updates neural network training statistics when learning messages.
    /// 
    /// # Arguments
    /// 
    /// * `learning_type` - Either "spam" or "ham"
    /// * `message_id` - The unique identifier for the message
    /// * `content` - The message content for feature extraction
    /// 
    /// # Returns
    /// 
    /// A `Result<()>` indicating success or failure of the update operation.
    async fn update_neural_training_stats(&self, learning_type: &str, message_id: &str, content: &str) -> Result<()> {
        let neural_manager = NeuralManager::new()?;
        let mut conn = self.redis_client.get_connection()?;
        
        // Update neural network statistics
        let now = Utc::now().to_rfc3339();
        
        // Increment total messages
        let _: i64 = conn.hincr(neural::NEURAL_STATS_KEY, "total_messages", 1)?;
        
        // Increment specific message type counter
        match learning_type {
            "spam" => {
                let _: i64 = conn.hincr(neural::NEURAL_STATS_KEY, "spam_messages", 1)?;
                log::info!("Neural network: Added spam message to training dataset");
            }
            "ham" => {
                let _: i64 = conn.hincr(neural::NEURAL_STATS_KEY, "ham_messages", 1)?;
                log::info!("Neural network: Added ham message to training dataset");
            }
            _ => {
                log::warn!("Neural network: Unknown learning type '{}'", learning_type);
                return Ok(());
            }
        }
        
        // Update last training timestamp
        let _: () = conn.hset(neural::NEURAL_STATS_KEY, "last_training", &now)?;
        
        // Store message features for neural network training
        self.store_neural_features(message_id, content, learning_type).await?;
        
        // Check if neural network is ready and log status
        match neural_manager.is_ready() {
            Ok(true) => {
                log::info!("Neural network is ready, {} learning will contribute to model training", learning_type);
                
                // Increment training iterations when network is ready
                let _: i64 = conn.hincr(neural::NEURAL_STATS_KEY, "training_iterations", 1)?;
            }
            Ok(false) => {
                log::info!("Neural network is still training, {} learning will help build dataset", learning_type);
            }
            Err(e) => {
                log::warn!("Could not check neural network readiness: {}", e);
            }
        }
        
        Ok(())
    }
    
    /// Stores message features for neural network training.
    /// 
    /// # Arguments
    /// 
    /// * `message_id` - The unique identifier for the message
    /// * `content` - The message content to extract features from
    /// * `learning_type` - Either "spam" or "ham"
    /// 
    /// # Returns
    /// 
    /// A `Result<()>` indicating success or failure of the storage operation.
    async fn store_neural_features(&self, message_id: &str, content: &str, learning_type: &str) -> Result<()> {
        let mut conn = self.redis_client.get_connection()?;
        
        // Extract basic text features
        let features = self.extract_text_features(content);
        
        // Create feature record
        let feature_key = format!("{}:{}", neural::NEURAL_FEATURES_KEY, message_id);
        let feature_data = serde_json::json!({
            "message_id": message_id,
            "content_length": content.len(),
            "word_count": features.word_count,
            "link_count": features.link_count,
            "emoji_count": features.emoji_count,
            "caps_ratio": features.caps_ratio,
            "learning_type": learning_type,
            "timestamp": Utc::now().to_rfc3339(),
            "features": features
        });
        
        // Store features with expiration (7 days)
        let _: () = conn.set_ex(&feature_key, feature_data.to_string(), 7 * 24 * 60 * 60)?;
        
        log::debug!("Stored neural features for message {}: {:?}", message_id, features);
        Ok(())
    }
    
    /// Extracts text features for neural network training.
    /// 
    /// # Arguments
    /// 
    /// * `content` - The message content to analyze
    /// 
    /// # Returns
    /// 
    /// A `TextFeatures` struct containing extracted features.
    fn extract_text_features(&self, content: &str) -> TextFeatures {
        let word_count = content.split_whitespace().count();
        let link_count = content.matches("http").count() + content.matches("www").count();
        let emoji_count = content.chars().filter(|c| {
            // Simple emoji detection - check for emoji ranges
            let code = *c as u32;
            (code >= 0x1F600 && code <= 0x1F64F) || // Emoticons
            (code >= 0x1F300 && code <= 0x1F5FF) || // Miscellaneous Symbols and Pictographs
            (code >= 0x1F680 && code <= 0x1F6FF) || // Transport and Map Symbols
            (code >= 0x1F1E0 && code <= 0x1F1FF) || // Regional Indicator Symbols
            (code >= 0x2600 && code <= 0x26FF) ||   // Miscellaneous Symbols
            (code >= 0x2700 && code <= 0x27BF)      // Dingbats
        }).count();
        
        let caps_count = content.chars().filter(|c| c.is_uppercase()).count();
        let total_chars = content.chars().filter(|c| c.is_alphabetic()).count();
        let caps_ratio = if total_chars > 0 {
            caps_count as f64 / total_chars as f64
        } else {
            0.0
        };
        
        TextFeatures {
            word_count,
            link_count,
            emoji_count,
            caps_ratio,
        }
    }
    
    /// Gets Bayesian classifier statistics from Redis.
    /// 
    /// # Returns
    /// 
    /// A `Result<HashMap<String, i64>>` containing statistics about the classifier.
    pub fn get_bayes_stats(&self) -> Result<HashMap<String, i64>> {
        let mut conn = self.redis_client.get_connection()?;
        let mut stats = HashMap::new();
        
        // Get spam token count
        let spam_tokens: i64 = conn.scard(bayes::BAYES_SPAM_KEY)?;
        stats.insert("spam_tokens".to_string(), spam_tokens);
        
        // Get ham token count
        let ham_tokens: i64 = conn.scard(bayes::BAYES_HAM_KEY)?;
        stats.insert("ham_tokens".to_string(), ham_tokens);
        
        // Get total learned messages
        let spam_messages: i64 = conn.get(bayes::BAYES_SPAM_MESSAGES_KEY).unwrap_or(0);
        let ham_messages: i64 = conn.get(bayes::BAYES_HAM_MESSAGES_KEY).unwrap_or(0);
        stats.insert("spam_messages".to_string(), spam_messages);
        stats.insert("ham_messages".to_string(), ham_messages);
        
        // Calculate total messages
        let total_messages = spam_messages + ham_messages;
        stats.insert("total_messages".to_string(), total_messages);
        
        // Calculate spam ratio (percentage)
        let spam_ratio = if total_messages > 0 {
            (spam_messages * 100) / total_messages
        } else {
            0
        };
        stats.insert("spam_ratio_percent".to_string(), spam_ratio);
        
        Ok(stats)
    }
    
    /// Checks if the Bayesian classifier is ready for effective classification.
    /// 
    /// The classifier is considered ready when it has learned at least the minimum
    /// number of spam and ham messages as defined in the configuration.
    /// 
    /// # Returns
    /// 
    /// A `Result<bool>` indicating whether the classifier is ready.
    pub fn is_ready(&self) -> Result<bool> {
        let stats = self.get_bayes_stats()?;
        let spam_messages = stats.get("spam_messages").unwrap_or(&0);
        let ham_messages = stats.get("ham_messages").unwrap_or(&0);
        
        Ok(*spam_messages >= bayes::MIN_SPAM_MESSAGES && *ham_messages >= bayes::MIN_HAM_MESSAGES)
    }
    
    /// Checks if a message has already been learned.
    /// 
    /// # Arguments
    /// 
    /// * `message_id` - The unique identifier for the message
    /// 
    /// # Returns
    /// 
    /// A `Result<bool>` indicating whether the message has been learned.
    pub fn is_message_learned(&self, message_id: &str) -> Result<bool> {
        let mut conn = self.redis_client.get_connection()?;
        
        // Check both spam and ham learned records
        let spam_key = format!("{}spam:{}", bayes::BAYES_LEARNED_PREFIX, message_id);
        let ham_key = format!("{}ham:{}", bayes::BAYES_LEARNED_PREFIX, message_id);
        
        let spam_exists: bool = conn.exists(&spam_key)?;
        let ham_exists: bool = conn.exists(&ham_key)?;
        
        Ok(spam_exists || ham_exists)
    }
    
    /// Gets the learning type for a specific message.
    /// 
    /// # Arguments
    /// 
    /// * `message_id` - The unique identifier for the message
    /// 
    /// # Returns
    /// 
    /// A `Result<Option<String>>` containing "spam", "ham", or None if not learned.
    pub fn get_message_learning_type(&self, message_id: &str) -> Result<Option<String>> {
        let mut conn = self.redis_client.get_connection()?;
        
        let spam_key = format!("{}spam:{}", bayes::BAYES_LEARNED_PREFIX, message_id);
        let ham_key = format!("{}ham:{}", bayes::BAYES_LEARNED_PREFIX, message_id);
        
        let spam_exists: bool = conn.exists(&spam_key)?;
        let ham_exists: bool = conn.exists(&ham_key)?;
        
        if spam_exists {
            Ok(Some("spam".to_string()))
        } else if ham_exists {
            Ok(Some("ham".to_string()))
        } else {
            Ok(None)
        }
    }
    
    /// Validates message content before learning.
    /// 
    /// # Arguments
    /// 
    /// * `message_id` - The unique identifier for the message
    /// * `content` - The message content to validate
    /// 
    /// # Returns
    /// 
    /// A `Result<()>` indicating whether the content is valid for learning.
    pub fn validate_content_for_learning(&self, message_id: &str, content: &str) -> Result<()> {
        if content.trim().is_empty() {
            return Err(anyhow::anyhow!("Cannot learn empty message content for message ID: {}", message_id));
        }
        
        if content.len() < 10 {
            return Err(anyhow::anyhow!(
                "Message content too short ({} chars) for learning. Minimum 10 characters required. Content: '{}'",
                content.len(),
                content
            ));
        }
        
        // Check if message is already learned
        if let Ok(is_learned) = self.is_message_learned(message_id) {
            if is_learned {
                if let Ok(learning_type) = self.get_message_learning_type(message_id) {
                    if let Some(learned_as) = learning_type {
                        return Err(anyhow::anyhow!(
                            "Message {} has already been learned as {}. Cannot learn it again.",
                            message_id,
                            learned_as
                        ));
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Resets all Bayesian classifier data.
    /// 
    /// This will clear all learned tokens, message counters, and learning records.
    /// Use with caution as this will require retraining the classifier.
    /// 
    /// # Returns
    /// 
    /// A `Result<()>` indicating success or failure of the reset operation.
    pub fn reset_all_data(&self) -> Result<()> {
        let mut conn = self.redis_client.get_connection()?;
        
        // Clear token sets
        let _: () = conn.del(bayes::BAYES_SPAM_KEY)?;
        let _: () = conn.del(bayes::BAYES_HAM_KEY)?;
        
        // Clear message counters
        let _: () = conn.del(bayes::BAYES_SPAM_MESSAGES_KEY)?;
        let _: () = conn.del(bayes::BAYES_HAM_MESSAGES_KEY)?;
        
        // Clear learning records (this will clear all keys with the learned prefix)
        // Note: This is a simplified approach. In production, you might want to
        // iterate through all keys with the prefix and delete them individually.
        let pattern = format!("{}*", bayes::BAYES_LEARNED_PREFIX);
        let keys: Vec<String> = conn.keys(&pattern)?;
        if !keys.is_empty() {
            let _: () = conn.del(&keys)?;
        }
        
        log::info!("Successfully reset all Bayesian classifier data");
        Ok(())
    }
    
    /// Gets detailed classifier information including readiness status.
    /// 
    /// # Returns
    /// 
    /// A `Result<HashMap<String, String>>` containing detailed classifier information.
    pub fn get_detailed_info(&self) -> Result<HashMap<String, String>> {
        let stats = self.get_bayes_stats()?;
        let is_ready = self.is_ready()?;
        
        let mut info = HashMap::new();
        
        // Basic stats
        info.insert("spam_tokens".to_string(), stats.get("spam_tokens").unwrap_or(&0).to_string());
        info.insert("ham_tokens".to_string(), stats.get("ham_tokens").unwrap_or(&0).to_string());
        info.insert("spam_messages".to_string(), stats.get("spam_messages").unwrap_or(&0).to_string());
        info.insert("ham_messages".to_string(), stats.get("ham_messages").unwrap_or(&0).to_string());
        info.insert("total_messages".to_string(), stats.get("total_messages").unwrap_or(&0).to_string());
        info.insert("spam_ratio_percent".to_string(), stats.get("spam_ratio_percent").unwrap_or(&0).to_string());
        
        // Status information
        info.insert("is_ready".to_string(), is_ready.to_string());
        info.insert("status".to_string(), if is_ready { "Ready".to_string() } else { "Training".to_string() });
        
        // Requirements
        info.insert("min_spam_required".to_string(), bayes::MIN_SPAM_MESSAGES.to_string());
        info.insert("min_ham_required".to_string(), bayes::MIN_HAM_MESSAGES.to_string());
        
        // Progress indicators
        let spam_progress = if bayes::MIN_SPAM_MESSAGES > 0 {
            (stats.get("spam_messages").unwrap_or(&0) * 100) / bayes::MIN_SPAM_MESSAGES
        } else {
            0
        };
        let ham_progress = if bayes::MIN_HAM_MESSAGES > 0 {
            (stats.get("ham_messages").unwrap_or(&0) * 100) / bayes::MIN_HAM_MESSAGES
        } else {
            0
        };
        
        info.insert("spam_progress_percent".to_string(), spam_progress.to_string());
        info.insert("ham_progress_percent".to_string(), ham_progress.to_string());
        
        Ok(info)
    }
    
    /// Gets combined Bayesian and Neural Network statistics.
    /// 
    /// # Returns
    /// 
    /// A `Result<CombinedStats>` containing both Bayesian and Neural Network statistics.
    pub fn get_combined_stats(&self) -> Result<CombinedStats> {
        let bayes_stats = self.get_bayes_stats()?;
        let neural_manager = NeuralManager::new()?;
        let neural_stats = neural_manager.get_neural_stats()?;
        
        Ok(CombinedStats {
            bayes: bayes_stats,
            neural: neural_stats,
        })
    }
    
    /// Checks if both Bayesian and Neural Network classifiers are ready.
    /// 
    /// # Returns
    /// 
    /// A `Result<ClassifierReadiness>` containing readiness status for both classifiers.
    pub fn check_classifier_readiness(&self) -> Result<ClassifierReadiness> {
        let bayes_ready = self.is_ready()?;
        let neural_manager = NeuralManager::new()?;
        let neural_ready = neural_manager.is_ready()?;
        
        Ok(ClassifierReadiness {
            bayes_ready,
            neural_ready,
            both_ready: bayes_ready && neural_ready,
        })
    }
    
    /// Gets detailed information about both classifiers.
    /// 
    /// # Returns
    /// 
    /// A `Result<DetailedClassifierInfo>` containing comprehensive classifier information.
    pub fn get_detailed_classifier_info(&self) -> Result<DetailedClassifierInfo> {
        let bayes_info = self.get_detailed_info()?;
        let neural_manager = NeuralManager::new()?;
        let neural_stats = neural_manager.get_neural_stats()?;
        let neural_ready = neural_manager.is_ready()?;
        
        let mut neural_info = HashMap::new();
        neural_info.insert("total_messages".to_string(), neural_stats.total_messages.to_string());
        neural_info.insert("spam_messages".to_string(), neural_stats.spam_messages.to_string());
        neural_info.insert("ham_messages".to_string(), neural_stats.ham_messages.to_string());
        neural_info.insert("training_iterations".to_string(), neural_stats.training_iterations.to_string());
        neural_info.insert("model_accuracy".to_string(), format!("{:.2}", neural_stats.model_accuracy * 100.0));
        neural_info.insert("is_ready".to_string(), neural_ready.to_string());
        neural_info.insert("status".to_string(), if neural_ready { "Ready".to_string() } else { "Training".to_string() });
        neural_info.insert("min_samples_required".to_string(), neural::MIN_SAMPLES_REQUIRED.to_string());
        
        // Calculate neural network progress
        let progress = if neural::MIN_SAMPLES_REQUIRED > 0 {
            (neural_stats.total_messages * 100) / neural::MIN_SAMPLES_REQUIRED
        } else {
            0
        };
        neural_info.insert("progress_percent".to_string(), progress.to_string());
        
        Ok(DetailedClassifierInfo {
            bayes: bayes_info,
            neural: neural_info,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    static INIT: Once = Once::new();

    fn setup() {
        INIT.call_once(|| {
            pretty_env_logger::init();
        });
    }

    #[tokio::test]
    async fn test_bayes_manager_creation() {
        setup();
        let bayes = BayesManager::new();
        assert!(bayes.is_ok());
    }

    #[tokio::test]
    async fn test_bayes_stats_retrieval() {
        setup();
        let bayes = BayesManager::new().unwrap();
        let stats = bayes.get_bayes_stats();
        assert!(stats.is_ok());
        
        let stats = stats.unwrap();
        assert!(stats.contains_key("spam_tokens"));
        assert!(stats.contains_key("ham_tokens"));
        assert!(stats.contains_key("spam_messages"));
        assert!(stats.contains_key("ham_messages"));
        assert!(stats.contains_key("total_messages"));
    }

    #[tokio::test]
    async fn test_bayes_readiness_check() {
        setup();
        let bayes = BayesManager::new().unwrap();
        let is_ready = bayes.is_ready();
        assert!(is_ready.is_ok());
        // The result depends on the current state of the classifier
        let _is_ready = is_ready.unwrap();
    }

    #[tokio::test]
    async fn test_message_learning_tracking() {
        setup();
        let bayes = BayesManager::new().unwrap();
        let test_message_id = "test_message_123";
        
        // Initially, the message should not be learned
        let is_learned = bayes.is_message_learned(test_message_id);
        assert!(is_learned.is_ok());
        assert!(!is_learned.unwrap());
        
        // Learning type should be None for unlearned message
        let learning_type = bayes.get_message_learning_type(test_message_id);
        assert!(learning_type.is_ok());
        assert!(learning_type.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_detailed_info_retrieval() {
        setup();
        let bayes = BayesManager::new().unwrap();
        let info = bayes.get_detailed_info();
        assert!(info.is_ok());
        
        let info = info.unwrap();
        assert!(info.contains_key("is_ready"));
        assert!(info.contains_key("status"));
        assert!(info.contains_key("min_spam_required"));
        assert!(info.contains_key("min_ham_required"));
        assert!(info.contains_key("spam_progress_percent"));
        assert!(info.contains_key("ham_progress_percent"));
    }
}
