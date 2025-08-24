use redis::Commands;
use reqwest::Client;
use std::collections::HashMap;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use crate::config::{rspamd, neural};

#[derive(Debug, Serialize, Deserialize)]
pub struct NeuralStats {
    pub total_messages: i64,
    pub spam_messages: i64,
    pub ham_messages: i64,
    pub training_iterations: i64,
    pub model_accuracy: f64,
    pub last_training: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NeuralFeatures {
    pub symbols: HashMap<String, f64>,
    pub metadata: HashMap<String, f64>,
    pub text_features: HashMap<String, f64>,
}

/// Manages Neural Network operations for the Rspamd Telegram bot.
/// 
/// This struct provides functionality to:
/// - Monitor neural network training status
/// - Track neural network performance metrics
/// - Manage neural network feature extraction
/// - Handle neural network symbol interpretation
pub struct NeuralManager {
    redis_client: redis::Client,
    rspamd_client: Client,
    rspamd_url: String,
    rspamd_password: String,
}

impl NeuralManager {
    /// Creates a new NeuralManager instance.
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
    
    /// Gets neural network statistics from Redis.
    pub fn get_neural_stats(&self) -> Result<NeuralStats> {
        let mut conn = self.redis_client.get_connection()?;
        
        // Get values with defaults for missing keys
        let total_messages: i64 = conn.hget(neural::NEURAL_STATS_KEY, "total_messages").unwrap_or(0);
        let spam_messages: i64 = conn.hget(neural::NEURAL_STATS_KEY, "spam_messages").unwrap_or(0);
        let ham_messages: i64 = conn.hget(neural::NEURAL_STATS_KEY, "ham_messages").unwrap_or(0);
        let training_iterations: i64 = conn.hget(neural::NEURAL_STATS_KEY, "training_iterations").unwrap_or(0);
        let model_accuracy: f64 = conn.hget(neural::NEURAL_STATS_KEY, "model_accuracy").unwrap_or(0.0);
        let last_training: Option<String> = conn.hget(neural::NEURAL_STATS_KEY, "last_training").ok();
        
        Ok(NeuralStats {
            total_messages,
            spam_messages,
            ham_messages,
            training_iterations,
            model_accuracy,
            last_training,
        })
    }
    
    /// Checks if neural network is ready for classification.
    pub fn is_ready(&self) -> Result<bool> {
        let stats = self.get_neural_stats()?;
        
        // Neural network is ready if it has been trained with sufficient data
        Ok(stats.total_messages >= neural::MIN_SAMPLES_REQUIRED && 
           stats.training_iterations > 0)
    }
    
    /// Gets neural network model accuracy.
    pub fn get_accuracy(&self) -> Result<f64> {
        let stats = self.get_neural_stats()?;
        Ok(stats.model_accuracy)
    }
    
    /// Extracts neural network features from Rspamd scan result.
    pub fn extract_features(&self, scan_reply: &rspamd_client::protocol::RspamdScanReply) -> NeuralFeatures {
        let mut symbols = HashMap::new();
        let mut metadata = HashMap::new();
        let mut text_features = HashMap::new();
        
        // Extract symbol scores
        for (symbol, symbol_data) in &scan_reply.symbols {
            symbols.insert(symbol.clone(), symbol_data.score);
        }
        
        // Extract basic metadata features
        metadata.insert("score".to_string(), scan_reply.score);
        metadata.insert("required_score".to_string(), scan_reply.required_score);
        
        // Extract text-based features from symbols if available
        // Note: We don't have direct access to the original text, so we'll use symbol-based features
        text_features.insert("symbol_count".to_string(), scan_reply.symbols.len() as f64);
        text_features.insert("total_score".to_string(), scan_reply.score);
        
        NeuralFeatures {
            symbols,
            metadata,
            text_features,
        }
    }
    
    /// Checks if neural network symbols are present in scan result.
    pub fn has_neural_symbols(&self, scan_reply: &rspamd_client::protocol::RspamdScanReply) -> bool {
        scan_reply.symbols.keys().any(|symbol| {
            symbol.starts_with("NEURAL_")
        })
    }
    
    /// Gets neural network classification result.
    pub fn get_neural_classification(&self, scan_reply: &rspamd_client::protocol::RspamdScanReply) -> Option<String> {
        if scan_reply.symbols.contains_key("NEURAL_SPAM") {
            Some("spam".to_string())
        } else if scan_reply.symbols.contains_key("NEURAL_HAM") {
            Some("ham".to_string())
        } else if scan_reply.symbols.contains_key("NEURAL_UNCERTAIN") {
            Some("uncertain".to_string())
        } else {
            None
        }
    }
    
    /// Gets neural network confidence score.
    pub fn get_neural_confidence(&self, scan_reply: &rspamd_client::protocol::RspamdScanReply) -> Option<f64> {
        // Calculate confidence from symbol weights
        let neural_score: f64 = scan_reply.symbols.iter()
            .filter(|(symbol, _)| symbol.starts_with("NEURAL_"))
            .map(|(_, symbol_data)| symbol_data.score)
            .sum();
        
        Some(neural_score.abs())
    }
}
