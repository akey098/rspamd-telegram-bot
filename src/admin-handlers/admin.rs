use teloxide::{prelude::*, types::InlineKeyboardButton, types::InlineKeyboardMarkup};
use redis::Commands;
use std::error::Error;
use crate::commands::AdminCommand;

pub async fn handle_admin_command(bot: Bot, msg: Message, cmd: AdminCommand) -> ResponseResult<()> {
    let redis_client = redis::Client::open("redis://127.0.0.1/").expect("Failed to connect to Redis");
    let mut redis_conn = redis_client.get_connection().expect("Failed to get Redis connection");

    match cmd {
        AdminCommand::MakeAdmin => {
            let chat_id = msg.chat.id.0;
            let _: () = redis_conn.sadd("admin_chats", chat_id)
                .expect("Failed to add chat to admin_chats");

            let bot_chats: Vec<i64> = redis_conn.smembers("bot_chats")
                .unwrap_or_else(|_| Vec::new());


            let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();
            for chat in bot_chats {
                rows.push(vec![InlineKeyboardButton::callback(
                    format!("Chat: {}", chat),
                    chat.to_string(),
                )]);
            }
            let keyboard = InlineKeyboardMarkup::new(rows);

            bot.send_message(msg.chat.id, "Admin registered! Please select a chat to moderate:")
                .reply_markup(keyboard)
                .await?;
        }
        AdminCommand::Help => {
            bot.send_message(msg.chat.id, "Commands:\n/make_admin - register current chat as admin control chat\n…")
                .await?;
        }
        AdminCommand::Enable { feature } => {
            bot.send_message(msg.chat.id, format!("Feature '{}' enabled.", feature)).await?;
        }
        AdminCommand::Disable { feature } => {
            bot.send_message(msg.chat.id, format!("Feature '{}' disabled.", feature)).await?;
        }
        AdminCommand::Stats => {
            bot.send_message(msg.chat.id, "Stats: [Placeholder]").await?;
        }
        AdminCommand::Reputation { user } => {
            bot.send_message(msg.chat.id, format!("Reputation for {}: [Placeholder]", user)).await?;
        }
        AdminCommand::Recent => {
            bot.send_message(msg.chat.id, "Recent flagged messages: [Placeholder]").await?;
        }
        AdminCommand::AddRegex { pattern } => {
            bot.send_message(msg.chat.id, format!("Added regex pattern: {}", pattern)).await?;
        }
    }
    Ok(())
}
