use crate::config::{field, key, suffix, TRUSTED_MESSAGE_TTL, REPLY_TRACKING_TTL, reply_aware, rate_limit, selective_trust};
use chrono::{DateTime, Utc};
use redis::Commands;
use std::error::Error;
use teloxide::types::{ChatId, MessageId, UserId};

/// Types of trusted messages that can be replied to
#[derive(Debug, Clone, PartialEq)]
pub enum TrustedMessageType {
    /// Message sent by the bot itself
    Bot,
    /// Message sent by a chat admin
    Admin,
    /// Message sent by a verified user
    Verified,
}

impl TrustedMessageType {
    /// Convert to string representation for Redis storage
    pub fn as_str(&self) -> &'static str {
        match self {
            TrustedMessageType::Bot => "bot",
            TrustedMessageType::Admin => "admin",
            TrustedMessageType::Verified => "verified",
        }
    }

    /// Parse from string representation
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "bot" => Some(TrustedMessageType::Bot),
            "admin" => Some(TrustedMessageType::Admin),
            "verified" => Some(TrustedMessageType::Verified),
            _ => None,
        }
    }

    /// Get the score reduction for this type of trusted message
    pub fn score_reduction(&self) -> f64 {
        match self {
            TrustedMessageType::Bot => -3.0,    // Highest trust for bot messages
            TrustedMessageType::Admin => -2.0,   // Medium trust for admin messages
            TrustedMessageType::Verified => -1.0, // Lower trust for verified users
        }
    }
}

/// Metadata for a trusted message
#[derive(Debug, Clone)]
pub struct TrustedMessageMetadata {
    pub message_id: MessageId,
    pub chat_id: ChatId,
    pub sender_id: UserId,
    pub message_type: TrustedMessageType,
    pub timestamp: DateTime<Utc>,
}

impl TrustedMessageMetadata {
    /// Create new trusted message metadata
    pub fn new(
        message_id: MessageId,
        chat_id: ChatId,
        sender_id: UserId,
        message_type: TrustedMessageType,
    ) -> Self {
        Self {
            message_id,
            chat_id,
            sender_id,
            message_type,
            timestamp: Utc::now(),
        }
    }

    /// Get the Redis key for this trusted message
    pub fn redis_key(&self) -> String {
        format!("{}{}", key::TG_TRUSTED_PREFIX, self.message_id.0)
    }

    /// Get the metadata Redis key for this trusted message
    pub fn metadata_key(&self) -> String {
        format!("{}{}{}", self.redis_key(), suffix::TRUSTED_METADATA, self.message_id.0)
    }
}

/// Manager for handling trusted messages and reply tracking
pub struct TrustManager {
    redis_client: redis::Client,
}

impl TrustManager {
    /// Create a new TrustManager instance
    pub fn new(redis_url: &str) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let redis_client = redis::Client::open(redis_url)?;
        Ok(Self { redis_client })
    }

    /// Check if a user can create a trusted message (rate limiting)
    pub async fn can_create_trusted_message(&self, user_id: UserId) -> Result<bool, Box<dyn Error + Send + Sync>> {
        if !reply_aware::ENABLE_RATE_LIMITING {
            return Ok(true);
        }
        
        let mut conn = self.redis_client.get_connection()?;
        let rate_key = format!("{}{}", rate_limit::TRUSTED_MESSAGE_RATE_PREFIX, user_id.0);
        
        // Check current count
        let current_count: u32 = conn.get(&rate_key).unwrap_or(0);
        
        if current_count >= reply_aware::MAX_TRUSTED_MESSAGES_PER_HOUR {
            return Ok(false);
        }
        
        // Increment count with TTL
        conn.set_ex::<_, _, ()>(&rate_key, current_count + 1, reply_aware::TRUSTED_MESSAGE_RATE_WINDOW as u64)?;
        
        Ok(true)
    }

    /// Check if a user can create a trusted message without incrementing (for testing)
    pub async fn can_create_trusted_message_check_only(&self, user_id: UserId) -> Result<bool, Box<dyn Error + Send + Sync>> {
        if !reply_aware::ENABLE_RATE_LIMITING {
            return Ok(true);
        }
        
        let mut conn = self.redis_client.get_connection()?;
        let rate_key = format!("{}{}", rate_limit::TRUSTED_MESSAGE_RATE_PREFIX, user_id.0);
        
        // Check current count without incrementing
        let current_count: u32 = conn.get(&rate_key).unwrap_or(0);
        
        Ok(current_count < reply_aware::MAX_TRUSTED_MESSAGES_PER_HOUR)
    }

    /// Check if a user can reply to trusted messages (rate limiting)
    pub async fn can_reply_to_trusted(&self, user_id: UserId) -> Result<bool, Box<dyn Error + Send + Sync>> {
        if !reply_aware::ENABLE_RATE_LIMITING {
            return Ok(true);
        }
        
        let mut conn = self.redis_client.get_connection()?;
        let rate_key = format!("{}{}", rate_limit::REPLY_RATE_PREFIX, user_id.0);
        
        // Check current count
        let current_count: u32 = conn.get(&rate_key).unwrap_or(0);
        
        if current_count >= reply_aware::MAX_REPLIES_PER_HOUR {
            return Ok(false);
        }
        
        // Increment count with TTL
        conn.set_ex::<_, _, ()>(&rate_key, current_count + 1, reply_aware::REPLY_RATE_WINDOW as u64)?;
        
        Ok(true)
    }

    /// Check if a message should be trusted based on selective trusting rules
    pub async fn should_trust_message(&self, metadata: &TrustedMessageMetadata) -> Result<bool, Box<dyn Error + Send + Sync>> {
        if !reply_aware::ENABLE_SELECTIVE_TRUSTING {
            return Ok(true);
        }
        
        // Check message type restrictions
        match metadata.message_type {
            TrustedMessageType::Bot => {
                if !selective_trust::TRUST_BOT_MESSAGES {
                    return Ok(false);
                }
            }
            TrustedMessageType::Admin => {
                if !selective_trust::TRUST_ADMIN_MESSAGES {
                    return Ok(false);
                }
            }
            TrustedMessageType::Verified => {
                if !selective_trust::TRUST_VERIFIED_MESSAGES {
                    return Ok(false);
                }
            }
        }
        
        // Check reputation requirements
        if selective_trust::TRUST_GOOD_REPUTATION {
            let reputation = self.get_user_reputation(metadata.sender_id).await?;
            if reputation < selective_trust::MIN_REPUTATION_FOR_TRUST {
                return Ok(false);
            }
        }
        
        // Check message age
        if selective_trust::TRUST_RECENT_MESSAGES_ONLY {
            let age = Utc::now().timestamp() - metadata.timestamp.timestamp();
            if age > selective_trust::MAX_TRUSTED_MESSAGE_AGE as i64 {
                return Ok(false);
            }
        }
        
        Ok(true)
    }

    /// Get user reputation score
    pub async fn get_user_reputation(&self, user_id: UserId) -> Result<i64, Box<dyn Error + Send + Sync>> {
        let mut conn = self.redis_client.get_connection()?;
        let reputation_key = format!("tg:reputation:user:{}", user_id.0);
        
        let bad: i64 = conn.hget(&reputation_key, "bad").unwrap_or(0);
        let good: i64 = conn.hget(&reputation_key, "good").unwrap_or(0);
        
        Ok(bad - good) // Negative is good reputation
    }

    /// Mark a message as trusted with advanced checks
    pub async fn mark_trusted_advanced(&self, metadata: TrustedMessageMetadata) -> Result<bool, Box<dyn Error + Send + Sync>> {
        // Check rate limiting
        if !self.can_create_trusted_message(metadata.sender_id).await? {
            return Ok(false);
        }
        
        // Check selective trusting rules
        if !self.should_trust_message(&metadata).await? {
            return Ok(false);
        }
        
        // Mark as trusted
        self.mark_trusted(metadata).await?;
        Ok(true)
    }

    /// Check for spam patterns in reply content
    pub async fn check_reply_spam_patterns(&self, text: &str, user_id: UserId) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
        if !reply_aware::ENABLE_SPAM_MONITORING {
            return Ok(Vec::new());
        }
        
        let mut spam_patterns = Vec::new();
        
        // Check for excessive links
        let link_count = text.matches("http").count() + text.matches("https").count();
        if link_count > reply_aware::anti_evasion::MAX_LINKS_IN_REPLY as usize {
            spam_patterns.push("TG_REPLY_LINK_SPAM".to_string());
        }
        
        // Check for phone numbers
        let phone_regex = regex::Regex::new(r#"\+\d[\d\-\s\(\)]\d\d\d\d"#).unwrap();
        let phone_count = phone_regex.find_iter(text).count();
        if phone_count > reply_aware::anti_evasion::MAX_PHONE_NUMBERS_IN_REPLY as usize {
            spam_patterns.push("TG_REPLY_PHONE_SPAM".to_string());
        }
        
        // Check for invite links
        let invite_count = text.matches("t.me/joinchat").count() + text.matches("telegram.me/joinchat").count();
        if invite_count > reply_aware::anti_evasion::MAX_INVITE_LINKS_IN_REPLY as usize {
            spam_patterns.push("TG_REPLY_INVITE_SPAM".to_string());
        }
        
        // Check for excessive caps
        let total_chars = text.chars().filter(|c| c.is_alphabetic()).count();
        let caps_chars = text.chars().filter(|c| c.is_alphabetic() && c.is_uppercase()).count();
        if total_chars > 0 {
            let caps_ratio = caps_chars as f64 / total_chars as f64;
            if caps_ratio > reply_aware::anti_evasion::MAX_CAPS_RATIO_IN_REPLY {
                spam_patterns.push("TG_REPLY_CAPS_SPAM".to_string());
            }
        }
        
        // Check for excessive emoji
        let emoji_count = text.chars().filter(|c| {
            let code = *c as u32;
            (code >= 0x1F600 && code <= 0x1F64F) || // Emoticons
            (code >= 0x1F300 && code <= 0x1F5FF) || // Misc Symbols and Pictographs
            (code >= 0x1F680 && code <= 0x1F6FF) || // Transport and Map Symbols
            (code >= 0x1F1E0 && code <= 0x1F1FF)    // Regional Indicator Symbols
        }).count();
        if emoji_count > reply_aware::anti_evasion::MAX_EMOJI_IN_REPLY as usize {
            spam_patterns.push("TG_REPLY_EMOJI_SPAM".to_string());
        }
        
        // Track spam patterns for this user
        if !spam_patterns.is_empty() {
            self.track_spam_patterns(user_id, &spam_patterns).await?;
        }
        
        Ok(spam_patterns)
    }

    /// Track spam patterns for a user
    pub async fn track_spam_patterns(&self, user_id: UserId, patterns: &[String]) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut conn = self.redis_client.get_connection()?;
        let spam_key = format!("{}{}", rate_limit::SPAM_PATTERN_PREFIX, user_id.0);
        
        for pattern in patterns {
            conn.sadd::<_, _, ()>(&spam_key, pattern)?;
        }
        
        // Set TTL for spam tracking
        conn.expire::<_, ()>(&spam_key, 3600)?; // 1 hour
        
        Ok(())
    }

    /// Get spam pattern history for a user
    pub async fn get_spam_patterns(&self, user_id: UserId) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
        let mut conn = self.redis_client.get_connection()?;
        let spam_key = format!("{}{}", rate_limit::SPAM_PATTERN_PREFIX, user_id.0);
        
        let patterns: Vec<String> = conn.smembers(&spam_key).unwrap_or_default();
        Ok(patterns)
    }

    /// Calculate adjusted score reduction based on trust level and spam patterns
    pub async fn calculate_score_reduction(&self, metadata: &TrustedMessageMetadata, user_id: UserId) -> Result<f64, Box<dyn Error + Send + Sync>> {
        let mut reduction = metadata.message_type.score_reduction();
        
        // Check for spam patterns in user history
        let spam_patterns = self.get_spam_patterns(user_id).await?;
        if !spam_patterns.is_empty() {
            // Reduce the score reduction for users with spam history
            reduction = reduction * 0.5;
        }
        
        // Ensure we don't exceed maximum reduction
        if reduction < reply_aware::MAX_SCORE_REDUCTION {
            reduction = reply_aware::MAX_SCORE_REDUCTION;
        }
        
        Ok(reduction)
    }

    /// Mark a message as trusted
    pub async fn mark_trusted(&self, metadata: TrustedMessageMetadata) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut conn = self.redis_client.get_connection()?;
        
        // Store the trusted message with TTL
        let key = metadata.redis_key();
        conn.set_ex::<_, _, ()>(&key, "1", TRUSTED_MESSAGE_TTL as u64)?;
        
        // Store metadata in a hash
        let metadata_key = metadata.metadata_key();
        let _: () = conn.hset_multiple(
            &metadata_key,
            &[
                (field::TRUSTED_SENDER, metadata.sender_id.0.to_string()),
                (field::TRUSTED_CHAT, metadata.chat_id.0.to_string()),
                (field::TRUSTED_TIMESTAMP, metadata.timestamp.timestamp().to_string()),
                (field::TRUSTED_TYPE, metadata.message_type.as_str().to_string()),
            ],
        )?;
        
        // Set TTL for metadata
        conn.expire::<_, ()>(&metadata_key, TRUSTED_MESSAGE_TTL as i64)?;
        
        Ok(())
    }

    /// Check if a message is trusted
    pub async fn is_trusted(&self, message_id: MessageId) -> Result<bool, Box<dyn Error + Send + Sync>> {
        let mut conn = self.redis_client.get_connection()?;
        let key = format!("{}{}", key::TG_TRUSTED_PREFIX, message_id.0);
        let exists: bool = conn.exists(&key)?;
        Ok(exists)
    }

    /// Get trusted message metadata
    pub async fn get_trusted_metadata(&self, message_id: MessageId) -> Result<Option<TrustedMessageMetadata>, Box<dyn Error + Send + Sync>> {
        let mut conn = self.redis_client.get_connection()?;
        let metadata_key = format!("{}{}{}{}", key::TG_TRUSTED_PREFIX, message_id.0, suffix::TRUSTED_METADATA, message_id.0);
        
        // Check if metadata exists
        let exists: bool = conn.exists(&metadata_key)?;
        if !exists {
            return Ok(None);
        }
        
        // Get metadata fields
        let sender_id: String = conn.hget(&metadata_key, field::TRUSTED_SENDER)?;
        let chat_id: String = conn.hget(&metadata_key, field::TRUSTED_CHAT)?;
        let timestamp: String = conn.hget(&metadata_key, field::TRUSTED_TIMESTAMP)?;
        let message_type: String = conn.hget(&metadata_key, field::TRUSTED_TYPE)?;
        
        let sender_id = sender_id.parse::<u64>()?;
        let chat_id = chat_id.parse::<i64>()?;
        let timestamp = timestamp.parse::<i64>()?;
        let message_type = TrustedMessageType::from_str(&message_type)
            .ok_or("Invalid message type")?;
        
        let metadata = TrustedMessageMetadata {
            message_id,
            chat_id: ChatId(chat_id),
            sender_id: UserId(sender_id),
            message_type,
            timestamp: DateTime::from_timestamp(timestamp, 0)
                .unwrap_or_else(|| Utc::now()),
        };
        
        Ok(Some(metadata))
    }

    /// Track a reply to a trusted message
    pub async fn track_reply(&self, chat_id: ChatId, reply_message_id: MessageId, trusted_message_id: MessageId) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut conn = self.redis_client.get_connection()?;
        let reply_key = format!("{}{}:{}:{}", key::TG_REPLIES_PREFIX, chat_id.0, trusted_message_id.0, reply_message_id.0);
        
        // Store reply tracking with TTL
        conn.set_ex::<_, _, ()>(&reply_key, "1", REPLY_TRACKING_TTL as u64)?;
        
        Ok(())
    }

    /// Check if a message is a reply to a trusted message
    pub async fn is_reply_to_trusted(&self, chat_id: ChatId, message_id: MessageId) -> Result<Option<TrustedMessageMetadata>, Box<dyn Error + Send + Sync>> {
        let mut conn = self.redis_client.get_connection()?;
        
        // Check if this message ID is tracked as a reply
        // Key format: tg:replies:<chat_id>:<trusted_message_id>:<reply_message_id>
        let pattern = format!("{}{}:*:{}", key::TG_REPLIES_PREFIX, chat_id.0, message_id.0);
        let keys: Vec<String> = conn.keys(&pattern)?;
        
        if keys.is_empty() {
            return Ok(None);
        }
        
        // Extract the trusted message ID from the reply key
        let reply_key = &keys[0];
        let parts: Vec<&str> = reply_key.split(':').collect();
        if parts.len() >= 4 {
            if let Ok(trusted_message_id) = parts[3].parse::<i32>() {
                return self.get_trusted_metadata(MessageId(trusted_message_id)).await;
            }
        }
        
        Ok(None)
    }

    /// Clean up expired trusted messages (called periodically)
    pub async fn cleanup_expired(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Redis automatically handles TTL cleanup, but we can add additional cleanup logic here
        // For example, cleaning up orphaned metadata entries
        Ok(())
    }

    /// Get statistics about trusted messages
    pub async fn get_stats(&self) -> Result<TrustStats, Box<dyn Error + Send + Sync>> {
        let mut conn = self.redis_client.get_connection()?;
        
        // Count trusted messages (only the main keys, not metadata keys)
        let trusted_pattern = format!("{}*", key::TG_TRUSTED_PREFIX);
        let all_trusted_keys: Vec<String> = conn.keys(&trusted_pattern)?;
        let trusted_messages = all_trusted_keys.iter()
            .filter(|key| !key.contains("metadata"))
            .count();
        
        // Count reply tracking entries
        let reply_pattern = format!("{}*", key::TG_REPLIES_PREFIX);
        let reply_keys: Vec<String> = conn.keys(&reply_pattern)?;
        
        Ok(TrustStats {
            trusted_messages,
            reply_tracking: reply_keys.len(),
        })
    }
}

/// Statistics about trusted messages and reply tracking
#[derive(Debug, Clone)]
pub struct TrustStats {
    pub trusted_messages: usize,
    pub reply_tracking: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use teloxide::types::{ChatId, MessageId, UserId};

    #[tokio::test]
    async fn test_trusted_message_lifecycle() {
        // This test would require a Redis instance
        // For now, we'll just test the struct methods
        let metadata = TrustedMessageMetadata::new(
            MessageId(123),
            ChatId(456),
            UserId(789),
            TrustedMessageType::Bot,
        );
        
        assert_eq!(metadata.message_id.0, 123);
        assert_eq!(metadata.chat_id.0, 456);
        assert_eq!(metadata.sender_id.0, 789);
        assert_eq!(metadata.message_type, TrustedMessageType::Bot);
        assert_eq!(metadata.message_type.as_str(), "bot");
        assert_eq!(metadata.message_type.score_reduction(), -3.0);
    }

    #[test]
    fn test_trusted_message_type() {
        assert_eq!(TrustedMessageType::Bot.as_str(), "bot");
        assert_eq!(TrustedMessageType::Admin.as_str(), "admin");
        assert_eq!(TrustedMessageType::Verified.as_str(), "verified");
        
        assert_eq!(TrustedMessageType::from_str("bot"), Some(TrustedMessageType::Bot));
        assert_eq!(TrustedMessageType::from_str("admin"), Some(TrustedMessageType::Admin));
        assert_eq!(TrustedMessageType::from_str("verified"), Some(TrustedMessageType::Verified));
        assert_eq!(TrustedMessageType::from_str("invalid"), None);
        
        assert_eq!(TrustedMessageType::Bot.score_reduction(), -3.0);
        assert_eq!(TrustedMessageType::Admin.score_reduction(), -2.0);
        assert_eq!(TrustedMessageType::Verified.score_reduction(), -1.0);
    }
} 