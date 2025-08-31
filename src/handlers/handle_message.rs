use crate::config::{field, key, symbol, bayes};
use crate::handlers::scan_msg;
use crate::trust_manager::TrustManager;
use crate::fuzzy_trainer::FuzzyTrainer;
use crate::bayes_manager::BayesManager;
use chrono::{Duration, Utc};
use redis::Commands;
use std::error::Error;
use teloxide::prelude::*;
use teloxide::types::{ChatPermissions, ChatMemberStatus};
use once_cell::sync::Lazy;

static FUZZY_TRAINER: Lazy<FuzzyTrainer> = Lazy::new(|| FuzzyTrainer::new());

async fn check_bot_permissions(bot: &Bot, chat_id: ChatId) -> anyhow::Result<()> {
    match bot.get_chat_member(chat_id, bot.get_me().await?.id).await {
        Ok(member) => {
            println!("Bot member status: {:?}", member.status());
            match member.status() {
                ChatMemberStatus::Administrator => {
                    println!("Bot is admin - should have restrict permissions");
                    Ok(())
                },
                ChatMemberStatus::Owner => {
                    println!("Bot is owner - has all permissions");
                    Ok(())
                },
                _ => {
                    Err(anyhow::anyhow!("Bot is not admin in this chat"))
                }
            }
        },
        Err(e) => {
            Err(anyhow::anyhow!("Failed to get bot member info: {}", e))
        }
    }
}

pub async fn handle_message(
    bot: Bot,
    message: Message,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let text = if let Some(text) = message.text() {
        text.to_string()
    } else {
        return Ok(());
    };
    
    // Store text for fuzzy training
    let text_for_fuzzy = text.clone();
    
    // Initialize trust manager for reply-aware filtering
    let _trust_manager = TrustManager::new("redis://127.0.0.1/")
        .map_err(|e| format!("Failed to create trust manager: {}", e))?;
    
    let result = scan_msg(message.clone(), text.clone()).await;
    let scan_result = match result {
        Ok(scan_result) => scan_result,
        Err(e) => {
            eprintln!("Failed to scan message: {}", e);
            return Ok(());
        }
    };
    
    // Store message content in Redis for learning commands
    let redis_client = redis::Client::open("redis://127.0.0.1/").expect("Failed to connect to Redis");
    let mut redis_conn = redis_client.get_connection().expect("Failed to get Redis connection");
    let message_key = format!("tg:message:{}", message.id.0);
    if let Err(e) = redis_conn.set_ex::<_, _, ()>(&message_key, &text, 86400) { // 24 hour TTL
        eprintln!("Failed to store message content in Redis: {}", e);
    }
    
    // Auto-learning integration for Bayesian classifier
    let bayes_manager = BayesManager::new();
    if let Ok(bayes) = bayes_manager {
        let score = scan_result.score;
        let message_id = message.id.0.to_string();
        // Auto-learn high-scoring messages as spam
        if score >= bayes::AUTOLEARN_SPAM_THRESHOLD {
            match bayes.learn_spam(&message_id, &text).await {
                Ok(()) => {
                    println!("Auto-learned message {} as spam (score: {})", message_id, score);
                }
                Err(e) => {
                    eprintln!("Failed to auto-learn message {} as spam: {}", message_id, e);
                }
            }
        }
        // Auto-learn very low-scoring messages as ham
        else if score <= bayes::AUTOLEARN_HAM_THRESHOLD {
            match bayes.learn_ham(&message_id, &text).await {
                Ok(()) => {
                    println!("Auto-learned message {} as ham (score: {})", message_id, score);
                }
                Err(e) => {
                    eprintln!("Failed to auto-learn message {} as ham: {}", message_id, e);
                }
            }
        }
    } else {
        eprintln!("Failed to create Bayes manager for auto-learning");
    }
    
    // Check for reputation symbols and adjust score
    let has_user_reputation = scan_result.symbols.iter().any(|s| s.0 == symbol::USER_REPUTATION);
    let has_bad_reputation = scan_result.symbols.iter().any(|s| s.0 == symbol::USER_REPUTATION_BAD);
    let has_good_reputation = scan_result.symbols.iter().any(|s| s.0 == symbol::USER_REPUTATION_GOOD);
    
    // Check for reply-aware filtering symbols
    let has_reply_symbol = scan_result.symbols.iter().any(|s| s.0 == symbol::TG_REPLY);
    let has_reply_bot = scan_result.symbols.iter().any(|s| s.0 == symbol::TG_REPLY_BOT);
    let has_reply_admin = scan_result.symbols.iter().any(|s| s.0 == symbol::TG_REPLY_ADMIN);
    let has_reply_verified = scan_result.symbols.iter().any(|s| s.0 == symbol::TG_REPLY_VERIFIED);
    
    // Adjust score based on reputation and reply context
    let mut adjusted_score = scan_result.score;
    if has_user_reputation {
        println!("User reputation symbol detected");
    }
    if has_bad_reputation {
        adjusted_score += 5.0; // Add penalty for bad reputation
        println!("User has bad reputation, adjusting score by +5.0");
    } else if has_good_reputation {
        adjusted_score -= 1.0; // Reduce score for good reputation
        println!("User has good reputation, adjusting score by -1.0");
    }
    
    // Apply reply-aware filtering adjustments
    if has_reply_symbol {
        println!("Reply symbol detected - applying contextual filtering");
        if has_reply_bot {
            adjusted_score -= 3.0; // Highest trust for replies to bot messages
            println!("Reply to bot message detected, adjusting score by -3.0");
        } else if has_reply_admin {
            adjusted_score -= 2.0; // Medium trust for replies to admin messages
            println!("Reply to admin message detected, adjusting score by -2.0");
        } else if has_reply_verified {
            adjusted_score -= 1.0; // Lower trust for replies to verified users
            println!("Reply to verified user message detected, adjusting score by -1.0");
        } else {
            adjusted_score -= 0.5; // Small reduction for any reply
            println!("General reply detected, adjusting score by -0.5");
        }
    }
    
    // Determine action based on adjusted score
    let action = if adjusted_score >= 15.0 {
        "tg_ban"
    } else if adjusted_score >= 10.0 {
        "tg_delete"
    } else if adjusted_score >= 5.0 {
        "tg_warn"
    } else {
        "none"
    };
    
    let user_id = message.from.unwrap().id;
    let chat_id = message.chat.id;
    let key = format!("{}{}", key::TG_CHATS_PREFIX, chat_id);
    let admin_chat_exists: bool = redis_conn
        .hexists(key.clone(), field::ADMIN_CHAT)
        .expect("Failed to check if admin chat exists");
    let mut admin_chat: Vec<i64> =  Vec::new();
    if admin_chat_exists {
        admin_chat = redis_conn
            .hget(key.clone(), field::ADMIN_CHAT)
            .expect("Failed to get admin chat");
    }

    // -------------------------------------------------------------
    // Map Rspamd actions to Telegram bot actions:
    // - add_header (score 5.0) -> tg_warn
    // - greylist (score 10.0) -> tg_delete
    // - reject (score 15.0) -> tg_ban (temporary ban, permanent after 3rd)
    // -------------------------------------------------------------
    
    match action {
        // Temporarily mute the user (ban) and delete the offending message
        "tg_ban" => {
            let _ = bot.delete_message(chat_id, message.id).await;
            println!(
                "Deleting message {} from chat {} and muting user {}.",
                message.id, chat_id, user_id
            );

            // Teach fuzzy storage after deletion
            if let Err(e) = FUZZY_TRAINER.teach_fuzzy(&text_for_fuzzy).await {
                eprintln!("Failed to teach fuzzy storage: {}", e);
            }

            // Get user key for Redis operations
            let user_key = format!("{}{}", key::TG_USERS_PREFIX, user_id);
            
            // Check current ban count and handle ban logic
            let banned_q: i64 = redis_conn
                .hget(&user_key, field::BANNED_Q)
                .unwrap_or(0);
            
            // Increment ban counter
            let _: () = redis_conn
                .hincr(&user_key, field::BANNED_Q, 1)
                .expect("Failed to increment ban counter");
            
            // If this is the 3rd ban, trigger permanent ban
            if banned_q >= 2 { // 0-indexed, so 2 means 3rd ban
                let _: () = redis_conn
                    .hset(&user_key, field::PERM_BANNED, "1")
                    .expect("Failed to set permanent ban");
                println!("User {} permanently banned after 3rd violation.", user_id);
            } else {
                // Set temporary ban with expiration
                let _: () = redis_conn
                    .expire(&user_key, 3600) // 1 hour ban
                    .expect("Failed to set ban expiration");
                println!("User {} temporarily banned for 1 hour.", user_id);
            }

            let notify_text = format!(
                "Banned user {} from chat {} for spam (message {}).",
                user_id, chat_id, message.id
            );
            if admin_chat_exists {
                bot.send_message(ChatId(admin_chat[0]), notify_text).await?;
            } else {
                bot.send_message(chat_id, notify_text).await?;
            }
        }

        // Delete message but do not ban the user
        "tg_delete" => {
            println!(
                "Deleting message {} from chat {} due to spam.",
                message.id, chat_id
            );
            bot.delete_message(chat_id, message.id).await?;
            
            // Teach fuzzy storage after deletion
            if let Err(e) = FUZZY_TRAINER.teach_fuzzy(&text_for_fuzzy).await {
                eprintln!("Failed to teach fuzzy storage: {}", e);
            }
            let _: () = redis_conn
                .hincr(key.clone(), field::DELETED, 1)
                .expect("Failed to update deleted count");

            let notify_text = format!(
                "Deleted message {} from user {} in chat {} for spam.",
                message.id, user_id, chat_id
            );
            if admin_chat_exists {
                bot.send_message(ChatId(admin_chat[0]), notify_text).await?;
            } else {
                bot.send_message(chat_id, notify_text).await?;
            }
        }

        // Just warn the user
        "tg_warn" => {
            println!(
                "Warning user {} in chat {} about spammy behavior.",
                user_id, chat_id
            );
            let notify_text = format!(
                "Warning: message {} from user {} in chat {} looks like spam.",
                message.id, user_id, chat_id
            );
            if admin_chat_exists {
                bot.send_message(ChatId(admin_chat[0]), notify_text).await?;
            } else {
                bot.send_message(chat_id, notify_text).await?;
            }
        }

        // Any other action: do nothing special
        _ => {
            println!("Message is ok");
        }
    }

    println!("Your score is {} and the action is {}", scan_result.score, scan_result.action);
    if adjusted_score != scan_result.score {
        println!("Adjusted score after reputation: {} (original: {})", adjusted_score, scan_result.score);
    }
    for symbol in scan_result.symbols {
        println!("Symbol: {} Score: {}", symbol.0, symbol.1.score);
    }
    Ok(())
}

async fn mute_user_for(
    bot: Bot,
    chat_id: ChatId,
    user_id: UserId,
    seconds: i64,
) -> anyhow::Result<()> {
    let until_ts = Utc::now() + Duration::seconds(seconds);
    println!("Setting mute until: {}", until_ts);

    let perms = ChatPermissions::empty();
    println!("Using empty permissions for mute");

    match bot.restrict_chat_member(chat_id, user_id, perms)
        .until_date(until_ts)
        .await {
        Ok(_) => {
            println!("Restrict chat member API call successful");
            Ok(())
        },
        Err(e) => {
            eprintln!("Restrict chat member API call failed: {}", e);
            Err(anyhow::anyhow!("Failed to restrict chat member: {}", e))
        }
    }
}