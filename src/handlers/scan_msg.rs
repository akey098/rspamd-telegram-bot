use chrono::Utc;
use rspamd_client::{config::Config, error::RspamdError, protocol::RspamdScanReply, scan_async};
use teloxide::prelude::*;
use get_if_addrs::{get_if_addrs, IfAddr};
use crate::trust_manager::{TrustManager, TrustedMessageType};
use crate::neural_manager::NeuralManager;
use crate::config::{neural, symbol};
use log;
use std::collections::HashMap;

/// Scan a Telegram message: real Rspamd first, heuristic fallback.
pub async fn scan_msg(msg: Message, text: String) -> Result<RspamdScanReply, RspamdError> {
    let user = msg.from.as_ref().ok_or_else(|| RspamdError::ConfigError("Message has no sender".to_string()))?;
    let user_id = user.id.to_string();
    let user_name = user.username.as_deref().unwrap_or("anonymous").to_string();
    let chat_id = msg.chat.id;
    let msg_id = msg.id.0.to_string();
    let date = Utc::now().to_rfc2822();
    let text = text;
    let ip = detect_local_ipv4().unwrap_or_else(|| "127.0.0.1/32".to_string());
    
    // Initialize trust manager
    let trust_manager = TrustManager::new("redis://127.0.0.1/")
        .unwrap_or_else(|_| panic!("Failed to create trust manager"));
    
    // Check if this is a reply to a trusted message
    let in_reply_to_header = if let Some(reply_to_message) = msg.reply_to_message() {
        // Check rate limiting for replies
        if !trust_manager.can_reply_to_trusted(user.id).await.unwrap_or(true) {
            // User is rate limited, treat as regular message
            format!("<rate_limited.{}.{}@telegram.com>", reply_to_message.id.0, chat_id.0)
        } else {
            // Check if the replied-to message is trusted
            match trust_manager.is_trusted(reply_to_message.id).await {
                Ok(true) => {
                    // Get metadata to determine the type of trusted message
                    if let Ok(Some(metadata)) = trust_manager.get_trusted_metadata(reply_to_message.id).await {
                        // Check for spam patterns in reply content
                        let spam_patterns = trust_manager.check_reply_spam_patterns(&text, user.id).await.unwrap_or_default();
                        
                        // Calculate adjusted score reduction
                        let score_reduction = trust_manager.calculate_score_reduction(&metadata, user.id).await.unwrap_or(metadata.message_type.score_reduction());
                        
                        // Track this reply
                        let _ = trust_manager.track_reply(chat_id, msg.id, reply_to_message.id).await;
                        
                        // Return appropriate In-Reply-To header based on message type and spam patterns
                        if spam_patterns.is_empty() {
                            match metadata.message_type {
                                TrustedMessageType::Bot => format!("<bot.{}.{}@telegram.com>", reply_to_message.id.0, chat_id.0),
                                TrustedMessageType::Admin => format!("<admin.{}.{}@telegram.com>", reply_to_message.id.0, chat_id.0),
                                TrustedMessageType::Verified => format!("<verified.{}.{}@telegram.com>", reply_to_message.id.0, chat_id.0),
                            }
                        } else {
                            // Include spam pattern info in header
                            format!("<spam_reply.{}.{}.{}@telegram.com>", reply_to_message.id.0, chat_id.0, spam_patterns.join(","))
                        }
                    } else {
                        format!("<unknown.{}.{}@telegram.com>", reply_to_message.id.0, chat_id.0)
                    }
                }
                Ok(false) => {
                    // Not a trusted message, but still a reply
                    format!("<reply.{}.{}@telegram.com>", reply_to_message.id.0, chat_id.0)
                }
                Err(_) => {
                    // Error checking trust status, treat as regular reply
                    format!("<reply.{}.{}@telegram.com>", reply_to_message.id.0, chat_id.0)
                }
            }
        }
    } else {
        // Not a reply
        String::new()
    };
    
    // Build email headers
    let mut headers = format!(
        "Received: from {ip} ({ip}) by localhost.localdomain with HTTP; {date}\r\n\
        Date: {date}\r\n\
        From: telegram{user_name}@example.com\r\n\
        To: telegram{chat_id}@example.com\r\n\
        Subject: Telegram message\r\n\
        Message-ID: <{msg_id}.{chat_id}@example.com>\r\n\
        X-Telegram-User: {user_id}\r\n\
        X-Telegram-Chat: {chat_id}\r\n\
        X-TG-User: {user_id}\r\n",
        date = date,
        ip = ip,
        user_name = user_name,
        user_id = user_id,
        chat_id = chat_id,
        msg_id = msg_id,
    );
    
    // Add In-Reply-To header if this is a reply
    if !in_reply_to_header.is_empty() {
        headers.push_str(&format!("In-Reply-To: {}\r\n", in_reply_to_header));
    }
    
    // Complete email format with headers and content
    let email = format!(
        "{headers}\
        MIME-Version: 1.0\r\n\
        Content-Type: text/plain; charset=UTF-8\r\n\
        Content-Transfer-Encoding: 8bit\r\n\
        \r\n\
        {text}\r\n",
        headers = headers,
        text = text.replace("\n", "\r\n")
    );
    
    let options = Config::builder()
        .base_url(std::env::var("RSPAMD_URL").unwrap_or_else(|_| "http://localhost:11333".to_string()))
        .build();
    scan_async(&options, email).await
}

/// Enhanced scan function that also returns reply information and advanced metrics
pub async fn scan_msg_with_advanced_info(msg: Message, text: String) -> Result<(RspamdScanReply, Option<String>, Vec<String>), RspamdError> {
    let user = msg.from.as_ref().ok_or_else(|| RspamdError::ConfigError("Message has no sender".to_string()))?;
    let user_id = user.id.to_string();
    let user_name = user.username.as_deref().unwrap_or("anonymous").to_string();
    let chat_id = msg.chat.id;
    let date = Utc::now().to_rfc2822();
    let text = text;
    let ip = detect_local_ipv4().unwrap_or_else(|| "127.0.0.1/32".to_string());
    
    // Initialize trust manager
    let trust_manager = TrustManager::new("redis://127.0.0.1/")
        .unwrap_or_else(|_| panic!("Failed to create trust manager"));
    
    // Check if this is a reply to a trusted message
    let (in_reply_to_header, reply_type, spam_patterns) = if let Some(reply_to_message) = msg.reply_to_message() {
        // Check rate limiting for replies
        if !trust_manager.can_reply_to_trusted(user.id).await.unwrap_or(true) {
            // User is rate limited, treat as regular message
            (
                format!("<rate_limited.{}.{}@telegram.com>", reply_to_message.id.0, chat_id.0),
                Some("rate_limited".to_string()),
                vec!["TG_RATE_LIMITED".to_string()]
            )
        } else {
            // Check if the replied-to message is trusted
            match trust_manager.is_trusted(reply_to_message.id).await {
                Ok(true) => {
                    // Get metadata to determine the type of trusted message
                    if let Ok(Some(metadata)) = trust_manager.get_trusted_metadata(reply_to_message.id).await {
                        // Check for spam patterns in reply content
                        let spam_patterns = trust_manager.check_reply_spam_patterns(&text, user.id).await.unwrap_or_default();
                        
                        // Calculate adjusted score reduction
                        let score_reduction = trust_manager.calculate_score_reduction(&metadata, user.id).await.unwrap_or(metadata.message_type.score_reduction());
                        
                        // Track this reply
                        let _ = trust_manager.track_reply(chat_id, msg.id, reply_to_message.id).await;
                        
                        // Return appropriate In-Reply-To header and reply type
                        let (header, reply_type) = if spam_patterns.is_empty() {
                            match metadata.message_type {
                                TrustedMessageType::Bot => (
                                    format!("<bot.{}.{}@telegram.com>", reply_to_message.id.0, chat_id.0),
                                    Some("bot".to_string())
                                ),
                                TrustedMessageType::Admin => (
                                    format!("<admin.{}.{}@telegram.com>", reply_to_message.id.0, chat_id.0),
                                    Some("admin".to_string())
                                ),
                                TrustedMessageType::Verified => (
                                    format!("<verified.{}.{}@telegram.com>", reply_to_message.id.0, chat_id.0),
                                    Some("verified".to_string())
                                ),
                            }
                        } else {
                            (
                                format!("<spam_reply.{}.{}.{}@telegram.com>", reply_to_message.id.0, chat_id.0, spam_patterns.join(",")),
                                Some("spam_reply".to_string())
                            )
                        };
                        
                        (header, reply_type, spam_patterns)
                    } else {
                        (
                            format!("<unknown.{}.{}@telegram.com>", reply_to_message.id.0, chat_id.0),
                            Some("unknown".to_string()),
                            Vec::new()
                        )
                    }
                }
                Ok(false) => {
                    // Not a trusted message, but still a reply
                    (
                        format!("<reply.{}.{}@telegram.com>", reply_to_message.id.0, chat_id.0),
                        Some("regular".to_string()),
                        Vec::new()
                    )
                }
                Err(_) => {
                    // Error checking trust status, treat as regular reply
                    (
                        format!("<reply.{}.{}@telegram.com>", reply_to_message.id.0, chat_id.0),
                        Some("error".to_string()),
                        Vec::new()
                    )
                }
            }
        }
    } else {
        // Not a reply
        (String::new(), None, Vec::new())
    };
    
    // Build email headers
    let mut headers = format!(
        "Received: from {ip} ({ip}) by localhost.localdomain with HTTP; {date}\r\n\
        Date: {date}\r\n\
        From: telegram{user_name}@example.com\r\n\
        To: telegram{chat_id}@example.com\r\n\
        Subject: Telegram message\r\n\
        Message-ID: <{user_id}.{chat_id}@example.com>\r\n\
        X-Telegram-User: {user_id}\r\n\
        X-Telegram-Chat: {chat_id}\r\n\
        X-TG-User: {user_id}\r\n",
        date = date,
        ip = ip,
        user_name = user_name,
        user_id = user_id,
        chat_id = chat_id,
    );
    
    // Add In-Reply-To header if this is a reply
    if !in_reply_to_header.is_empty() {
        headers.push_str(&format!("In-Reply-To: {}\r\n", in_reply_to_header));
    }
    
    // Complete email format with headers and content
    let email = format!(
        "{headers}\
        MIME-Version: 1.0\r\n\
        Content-Type: text/plain; charset=UTF-8\r\n\
        Content-Transfer-Encoding: 8bit\r\n\
        \r\n\
        {text}\r\n",
        headers = headers,
        text = text.replace("\n", "\r\n")
    );
    
    let options = Config::builder()
        .base_url(std::env::var("RSPAMD_URL").unwrap_or_else(|_| "http://localhost:11333".to_string()))
        .build();
    
    let scan_result = scan_async(&options, email).await?;
    
    // Process neural network results if available
    let neural_manager = NeuralManager::new();
    if let Ok(manager) = neural_manager {
        // Check for neural network symbols
        if manager.has_neural_symbols(&scan_result) {
            if let Some(classification) = manager.get_neural_classification(&scan_result) {
                log::info!("Neural network classification: {}", classification);
                
                // Extract features for analysis
                let features = manager.extract_features(&scan_result);
                log::debug!("Neural features: {:?}", features);
                
                // Check confidence
                if let Some(confidence) = manager.get_neural_confidence(&scan_result) {
                    if confidence >= neural::CONFIDENCE_THRESHOLD {
                        log::info!("High confidence neural classification: {} (confidence: {:.2})", 
                                  classification, confidence);
                    } else {
                        log::info!("Low confidence neural classification: {} (confidence: {:.2})", 
                                  classification, confidence);
                    }
                }
            }
        }
    }
    
    Ok((scan_result, reply_type, spam_patterns))
}

/// Helper function to check if a message has reply symbols (for testing)
pub async fn check_reply_symbols(msg: &Message) -> HashMap<String, f64> {
    let mut reply_symbols = HashMap::new();
    
    if let Some(reply_to_message) = msg.reply_to_message() {
        let trust_manager = TrustManager::new("redis://127.0.0.1/")
            .unwrap_or_else(|_| panic!("Failed to create trust manager"));
        
        // Check if the replied-to message is trusted
        if let Ok(true) = trust_manager.is_trusted(reply_to_message.id).await {
            if let Ok(Some(metadata)) = trust_manager.get_trusted_metadata(reply_to_message.id).await {
                // Add reply symbols based on message type
                reply_symbols.insert(symbol::TG_REPLY.to_string(), metadata.message_type.score_reduction());
                match metadata.message_type {
                    TrustedMessageType::Bot => {
                        reply_symbols.insert(symbol::TG_REPLY_BOT.to_string(), -3.0);
                    },
                    TrustedMessageType::Admin => {
                        reply_symbols.insert(symbol::TG_REPLY_ADMIN.to_string(), -2.0);
                    },
                    TrustedMessageType::Verified => {
                        reply_symbols.insert(symbol::TG_REPLY_VERIFIED.to_string(), -1.0);
                    },
                }
            }
        }
    }
    
    reply_symbols
}

pub fn detect_local_ipv4() -> Option<String> {
    if let Ok(ifaces) = get_if_addrs() {
        for iface in ifaces {
            if let IfAddr::V4(v4addr) = iface.addr {
                let ip = v4addr.ip;
                if !ip.is_loopback() {
                    return Some(format!("{}/32", ip));
                }
            }
        }
    }
    None
}