use crate::config::{field, key, BAN_COUNTER_REDUCTION_INTERVAL, symbol};
use crate::handlers::scan_msg;
use chrono::{Duration, Utc};
use redis::Commands;
use std::error::Error;
use teloxide::prelude::*;
use teloxide::types::{ChatPermissions, ChatMemberStatus};

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
    let result = scan_msg(message.clone(), text).await;
    let scan_result = match result {
        Ok(scan_result) => scan_result,
        Err(e) => {
            eprintln!("Failed to scan message: {}", e);
            return Ok(());
        }
    };
    
    // Check for reputation symbols and adjust score
    let has_user_reputation = scan_result.symbols.iter().any(|s| s.0 == symbol::USER_REPUTATION);
    let has_bad_reputation = scan_result.symbols.iter().any(|s| s.0 == symbol::USER_REPUTATION_BAD);
    let has_good_reputation = scan_result.symbols.iter().any(|s| s.0 == symbol::USER_REPUTATION_GOOD);
    
    // Adjust score based on reputation
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
    
    let redis_client =
        redis::Client::open("redis://127.0.0.1/").expect("Failed to connect to Redis");
    let mut redis_conn = redis_client
        .get_connection()
        .expect("Failed to get Redis connection");
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
                let _ = bot.ban_chat_member(chat_id, user_id).await;
                println!("User {} in chat {} permanently banned (3rd ban)", user_id, chat_id);
                
                let notify_text = format!("Permanently banned user {} in chat {} for spam (3rd ban)", user_id, chat_id);
                if admin_chat_exists {
                    bot.send_message(ChatId(admin_chat[0]), notify_text).await?;
                } else {
                    bot.send_message(chat_id, notify_text).await?;
                }
            } else {
                // Temporary ban - mute user for the configured duration
                let mute_duration = 3600; // 1 hour default mute duration
                println!("Attempting to mute user {} in chat {} for {} seconds", user_id, chat_id, mute_duration);
                
                // Check bot permissions first
                match check_bot_permissions(&bot, chat_id).await {
                    Ok(_) => println!("Bot has required permissions"),
                    Err(e) => {
                        eprintln!("Bot permission check failed: {}", e);
                        // Continue anyway - the mute might still work
                    }
                }
                
                match mute_user_for(bot.clone(), chat_id, user_id, mute_duration).await {
                    Ok(_) => println!("Successfully muted user {} in chat {}", user_id, chat_id),
                    Err(e) => eprintln!("Failed to mute user {} in chat {}: {}", user_id, chat_id, e),
                }
                
                // Set up automatic ban counter reduction
                let reduction_time = Utc::now().timestamp() + BAN_COUNTER_REDUCTION_INTERVAL;
                let _: () = redis_conn
                    .hset(&user_key, "ban_reduction_time", reduction_time.to_string())
                    .expect("Failed to set ban reduction time");
                
                let notify_text = format!(
                    "Deleted message {} from user {} in chat {}. Muted user for {} seconds. Ban count: {}/3",
                    message.id, user_id, chat_id, mute_duration, banned_q + 1
                );
                if admin_chat_exists {
                    bot.send_message(ChatId(admin_chat[0]), notify_text).await?;
                } else {
                    bot.send_message(chat_id, notify_text).await?;
                }
            }
        }

        // Permanently ban the user and remove the message (legacy - should not be used)
        "tg_perm_ban" => {
            let _ = bot.delete_message(chat_id, message.id).await;
            let _ = bot.ban_chat_member(chat_id, user_id).await;
            println!("User {} in chat {} permanently banned", user_id, chat_id);

            let notify_text = format!("Banned user {} in chat {} for spam", user_id, chat_id);
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
        println!("{} {} {} ", symbol.0, symbol.1.score, symbol.1.metric_score);
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