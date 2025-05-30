use crate::config::{field, key, symbol};
use crate::handlers::scan_msg;
use chrono::{Duration, Utc};
use redis::Commands;
use std::error::Error;
use teloxide::prelude::*;
use teloxide::types::ChatPermissions;

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
    let scan_result = result.ok().unwrap();
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
    if scan_result.symbols.contains_key(symbol::TG_PERM_BAN) {
        let _ = bot.delete_message(chat_id, message.id).await;
        let _ = bot.ban_chat_member(chat_id, user_id).await;
        println!("User {} in chat {} banned", user_id, chat_id);
        if admin_chat_exists {
            bot.send_message(
                ChatId(admin_chat[0]),
                format!(
                    "Banned user {} in chat {} for spam",
                    user_id, chat_id
                ),
            )
                .await?;
        } else {
            bot.send_message(
                chat_id,
                format!(
                    "Banned user {} in chat {} for spam",
                    user_id, chat_id
                ),
            )
                .await?;
        }
    }
    else if scan_result.symbols.contains_key(symbol::TG_BAN) {
        let _ = bot.delete_message(chat_id, message.id).await;
        println!(
            "Deleting message {} from chat {} because it appears to be spam.",
            message.id, chat_id
        );
        let ttl: i64 = redis_conn
            .httl(key.clone(), field::BANNED)
            .expect("Failed to check user's banned.");
        let _ = mute_user_for(bot.clone(), chat_id, user_id, ttl).await;
        println!("Muted user {} for {}", user_id, ttl);
        if admin_chat_exists {
            bot.send_message(
                ChatId(admin_chat[0]),
                format!(
                    "Deleted message {} from user {} in chat {}. Muted user {} for {}",
                    message.id, user_id, chat_id, user_id, ttl
                ),
            )
                .await?;
        } else {
            bot.send_message(
                chat_id,
                format!(
                    "Deleted message {} from user {} in chat {}. Muted user {} for {}",
                    message.id, user_id, chat_id, user_id, ttl
                ),
            )
                .await?;
        }
        
    } else if scan_result.score >= 10.0
        || scan_result.symbols.contains_key(symbol::TG_FLOOD)
        || scan_result.symbols.contains_key(symbol::TG_SUSPICIOUS)
    {
        println!(
            "Deleting message {} from chat {} because it appears to be spam.",
            message.id, chat_id
        );
        bot.delete_message(chat_id, message.id).await?;
        let _: () = redis_conn
            .hincr(key.clone(), field::DELETED, 1)
            .expect("Failed to update deleted count");
        if admin_chat_exists {
            bot.send_message(
                ChatId(admin_chat[0]),
                format!(
                    "Deleting message {} from user {} in chat {} for spam.",
                    message.id, user_id, chat_id
                ),
            )
            .await?;
        } else {
            bot.send_message(
                chat_id,
                format!(
                    "Deleting message {} from user {} in chat {} for spam.",
                    message.id, user_id, chat_id
                ),
            )
            .await?;
        }
    } else if scan_result.score >= 5.0 {
        println!(
            "Warning user {} in chat {} about spammy behavior.",
            user_id, chat_id
        );
        if admin_chat_exists {
            bot.send_message(
                ChatId(admin_chat[0]),
                format!(
                    "Warning message {} from user {} in chat {} is looks like spam.",
                    message.id, user_id, chat_id
                ),
            )
            .await?;
        } else {
            bot.send_message(
                chat_id,
                format!(
                    "Warning message {} from user {} in chat {} is looks like spam.",
                    message.id, user_id, chat_id
                ),
            )
            .await?;
        }
        
    } else {
        println!("Message is ok")
    }

    println!("Your score is {} and the action is {}", scan_result.score, scan_result.action);
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

    let perms = ChatPermissions::empty();

    bot.restrict_chat_member(chat_id, user_id, perms)
        .until_date(until_ts)
        .await?;

    Ok(())
}