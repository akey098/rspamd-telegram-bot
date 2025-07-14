use crate::config::{field, key};
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
    let scan_result = match result {
        Ok(scan_result) => scan_result,
        Err(e) => {
            eprintln!("Failed to scan message: {}", e);
            return Ok(());
        }
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
    // Rspamd **custom actions** integration
    // We now rely on the `action` field returned by Rspamd instead of
    // manually comparing scores or looking for specific symbols.
    // The expected custom actions should be defined in `local.d/actions.conf`,
    // e.g.:
    //   actions {
    //     tg_warn      = { score = 5.0;  }
    //     tg_delete    = { score = 10.0; }
    //     tg_ban       = { score = 12.0; }
    //     tg_perm_ban  = { score = 15.0; }
    //   }
    // -------------------------------------------------------------
    match scan_result.action.as_str() {
        // Permanently ban the user and remove the message
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

        // Temporarily mute the user (ban) and delete the offending message
        "tg_ban" => {
            let _ = bot.delete_message(chat_id, message.id).await;
            println!(
                "Deleting message {} from chat {} and muting user {}.",
                message.id, chat_id, user_id
            );

            let ttl: i64 = redis_conn
                .httl(key.clone(), field::BANNED)
                .expect("Failed to check user's banned.");
            let _ = mute_user_for(bot.clone(), chat_id, user_id, ttl).await;

            let notify_text = format!(
                "Deleted message {} from user {} in chat {}. Muted user for {} seconds",
                message.id, user_id, chat_id, ttl
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