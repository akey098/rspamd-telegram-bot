use redis::Commands;
use reqwest::Client;
use std::collections::HashMap;
use anyhow::Result;
use crate::config::{rspamd, bayes};

/// Manages Bayesian learning operations for the Rspamd Telegram bot.
/// 
/// This struct provides functionality to:
/// - Learn messages as spam or ham via Rspamd HTTP API
/// - Track learning statistics in Redis
/// - Monitor classifier readiness
/// - Manage learned message tracking
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
    
    /// Learns a message as spam via Rspamd HTTP API.
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
        let url = format!("{}/learnspam", self.rspamd_url);
        
        let response = self.rspamd_client
            .post(&url)
            .header("Password", &self.rspamd_password)
            .body(content.to_string())
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
            
            log::info!("Successfully learned message {} as spam", message_id);
            Ok(())
        } else {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            log::error!("Failed to learn as spam: {} - {}", status, error_text);
            Err(anyhow::anyhow!("Failed to learn as spam: {} - {}", status, error_text))
        }
    }
    
    /// Learns a message as ham via Rspamd HTTP API.
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
        let url = format!("{}/learnham", self.rspamd_url);
        
        let response = self.rspamd_client
            .post(&url)
            .header("Password", &self.rspamd_password)
            .body(content.to_string())
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
            
            log::info!("Successfully learned message {} as ham", message_id);
            Ok(())
        } else {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            log::error!("Failed to learn as ham: {} - {}", status, error_text);
            Err(anyhow::anyhow!("Failed to learn as ham: {} - {}", status, error_text))
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
