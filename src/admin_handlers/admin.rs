use crate::admin_handlers::AdminCommand;
use redis::{Commands, RedisResult};
use teloxide::types::{Chat, ChatMemberStatus};
use teloxide::{prelude::*, types::InlineKeyboardButton, types::InlineKeyboardMarkup};

async fn is_user_admin(bot: &Bot, chat: Chat, user_id: UserId) -> anyhow::Result<bool> {
    if !chat.is_private() {
        let member = bot.get_chat_member(chat.id, user_id).await?;
        match member.status() {
            ChatMemberStatus::Owner => Ok(true),
            ChatMemberStatus::Administrator => Ok(true),
            _ => Ok(false),
        }
    } else {
        Ok(true)
    }
}
pub async fn handle_admin_command(bot: Bot, msg: Message, cmd: AdminCommand) -> ResponseResult<()> {
    let redis_client =
        redis::Client::open("redis://127.0.0.1/").expect("Failed to connect to Redis");
    let mut redis_conn = redis_client
        .get_connection()
        .expect("Failed to get Redis connection");
    let user_id = msg.from.unwrap().id;
    let chat = msg.chat;
    let chat_id = chat.id;
    let is_admin = is_user_admin(&bot, chat, user_id).await.unwrap_or(false);
    if is_admin {
        match cmd {
            AdminCommand::MakeAdmin => {
                let _: () = redis_conn
                    .sadd(user_id.to_string() + ":admin_chats", chat_id.0)
                    .expect("Failed to add chat to admin_chats");
                
                let bot_chats: Vec<String> = redis_conn
                    .smembers(user_id.to_string() + ":bot_chats")
                    .unwrap_or_else(|_| Vec::new());

                let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();
                for chat in bot_chats {
                    rows.push(vec![InlineKeyboardButton::callback(
                        format!("Chat: {}", chat),
                        chat.to_string(),
                    )]);
                }
                let keyboard = InlineKeyboardMarkup::new(rows);

                bot.send_message(
                    chat_id,
                    "Admin chat registered! Please select chats to moderate:",
                )
                .reply_markup(keyboard)
                .await?;
            }
            AdminCommand::Help => {
                bot.send_message(
                    chat_id,
                    "Commands:\n/make_admin - register current chat as admin control chat\n…",
                )
                .await?;
            }
            AdminCommand::Enable { feature } => {
                bot.send_message(chat_id, format!("Feature '{}' enabled.", feature))
                    .await?;
            }
            AdminCommand::Disable { feature } => {
                bot.send_message(chat_id, format!("Feature '{}' disabled.", feature))
                    .await?;
            }
            AdminCommand::Stats => {
                bot.send_message(chat_id, "Stats: [Placeholder]").await?;
            }
            AdminCommand::Reputation { user } => {
                let key = format!("tg:{}:rep", user);

                let user_rep: RedisResult<i64> = redis_conn.get(&key);

                match user_rep {
                    Ok(rep) => {
                        bot.send_message(chat_id, format!("Reputation for {}: {}", user, rep))
                            .await?;
                    }
                    Err(_) => {
                        bot.send_message(chat_id, format!("Reputation for {}: 0", user))
                            .await?;
                    }
                }
            }
            AdminCommand::Recent => {
                bot.send_message(chat_id, "Recent flagged messages: [Placeholder]")
                    .await?;
            }
            AdminCommand::AddRegex { pattern } => {
                bot.send_message(chat_id, format!("Added regex pattern: {}", pattern))
                    .await?;
            }
        }
    } else {
        bot.send_message(chat_id, "You are not admin").await?;
    }
    Ok(())
}
