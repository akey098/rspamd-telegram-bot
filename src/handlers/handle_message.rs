use crate::handlers::scan_msg;
use redis::Commands;
use std::error::Error;
use teloxide::prelude::*;

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
    let key = format!("tg:chats:{}", chat_id);
    let admin_chat_exists: bool = redis_conn
        .hexists(key.clone(), "admin_chat")
        .expect("Failed to check if admin chat exists");
    let mut admin_chat: Vec<i64> =  Vec::new();
    if admin_chat_exists {
        admin_chat = redis_conn
            .hget(key.clone(), "admin_chat")
            .expect("Failed to get admin chat");
    }
    if scan_result.score >= 10.0
        || scan_result.symbols.contains_key("TG_FLOOD")
        || scan_result.symbols.contains_key("TG_SUSPICIOUS")
    {
        println!(
            "Deleting message {} from chat {} because it appears to be spam.",
            message.id, message.chat.id
        );
        bot.delete_message(message.chat.id, message.id).await?;
        let _: () = redis_conn
            .hincr(key.clone(), "deleted", 1)
            .expect("Failed to update deleted count");
        if admin_chat_exists {
            bot.send_message(
                ChatId(admin_chat[0]),
                format!(
                    "Deleting message {} from user {} in chat {} for spam.",
                    message.id, user_id, message.chat.id
                ),
            )
            .await?;
        } else {
            bot.send_message(
                message.chat.id,
                format!(
                    "Deleting message {} from user {} in chat {} for spam.",
                    message.id, user_id, message.chat.id
                ),
            )
            .await?;
        }
    } else if scan_result.score >= 5.0 {
        println!(
            "Warning user {} in chat {} about spammy behavior.",
            user_id, message.chat.id
        );
        if admin_chat_exists {
            bot.send_message(
                ChatId(admin_chat[0]),
                format!(
                    "Warning message {} from user {} in chat {} is looks like spam.",
                    message.id, user_id, message.chat.id
                ),
            )
            .await?;
        } else {
            bot.send_message(
                message.chat.id,
                format!(
                    "Warning message {} from user {} in chat {} is looks like spam.",
                    message.id, user_id, message.chat.id
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
