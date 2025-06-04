use crate::admin_handlers::AdminCommand;
use redis::{Commands, RedisResult};
use std::collections::HashMap;
use std::fmt::Write;
use teloxide::types::{Chat, ChatMemberStatus};
use teloxide::{prelude::*, types::InlineKeyboardButton, types::InlineKeyboardMarkup};
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use crate::config::{field, key, suffix};

use anyhow::Result;

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
                    .sadd(format!("{}{}", user_id.to_string(), suffix::ADMIN_CHATS), chat_id.0)
                    .expect("Failed to add chat to admin_chats");
                
                let bot_chats: Vec<i64> = redis_conn
                    .smembers(format!("{}{}", user_id.to_string(), suffix::BOT_CHATS))
                    .unwrap_or_else(|_| Vec::new());

                let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();
                for chat in bot_chats {
                    if chat == chat_id.0 { continue; }
                    let chat_name: String = redis_conn
                        .hget(format!("{}{}", key::TG_CHATS_PREFIX, chat), field::NAME)
                        .expect("Failed to get chat name");
                    rows.push(vec![InlineKeyboardButton::callback(
                        format!("Chat: {}", chat_name),
                        format!("makeadmin:{}", chat_id),
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
                    "Commands:\n/help - show help for commands\
                    \n/makeadmin - register current chat as admin control chat\n\
                    /reputation <username> - show user's reputation\n\
                    /addregex <symbol|pattern|score> - add regex rule to the rspamd\n\
                    /stats - show stats",
                )
                .await?;
            }
            AdminCommand::Enable { feature } => {
                
            }
            AdminCommand::Disable { feature } => {
                
            }
            AdminCommand::Stats => {
                let is_admin: bool = redis_conn
                    .sismember(format!("{}{}", user_id, suffix::ADMIN_CHATS), chat_id.0)
                    .expect("Failed to get admin chat");
                if !is_admin {
                    let stats: HashMap<String, String> = redis_conn
                        .hgetall(format!("{}{}", key::TG_CHATS_PREFIX, chat_id.0))
                        .expect("Failed to get chat stats");
                    let mut response = String::new();
                    for (field, value) in stats {
                        if field == field::NAME || field == field::ADMIN_CHAT {
                            continue;
                        }
                        writeln!(&mut response, "{}: {}", field, value).unwrap();
                    }
                    bot.send_message(chat_id, response).await?;
                } else {
                    let chats: Vec<i64> = redis_conn
                        .smembers(format!("admin:{}:moderated_chats", chat_id.0))
                        .expect("Failed to get moderated chats");
                    let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();
                    for chat in chats {
                        let chat_name: String = redis_conn
                            .hget(format!("{}{}", key::TG_CHATS_PREFIX, chat), field::NAME)
                            .expect("Failed to get chat name");
                        rows.push(vec![InlineKeyboardButton::callback(
                            format!("Chat: {}", chat_name),
                            format!("stats:{}", chat_id),
                        )]);
                    }
                    let keyboard = InlineKeyboardMarkup::new(rows);
                    bot.send_message(
                        chat_id,
                        "Which chat's stats do you want to see:",
                    )
                    .reply_markup(keyboard)
                    .await?;
                }
            }
            AdminCommand::Reputation { user } => {
                let key = format!("{}{}", key::TG_USERS_PREFIX, user);

                let user_rep: RedisResult<i64> = redis_conn.hget(key.clone(), field::REP);

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
            AdminCommand::AddRegex { pattern } => {
                let parts: Vec<&str> = pattern.split('|').map(str::trim).collect();
                if parts.len() != 3 {
                    bot.send_message(chat_id, "Usage: /addregex symbol|pattern|score").await?;
                    return Ok(());
                }
                let (symbol, regex_pattern, score) = (parts[0], parts[1], parts[2]);
                
                let lua_rule = format!(
                    "config['regexp']['{}'] = {{
                        re = '{}',
                        score = {},
                        condition = function(task)
                            if task:get_header('Subject') then
                                return true
                            end
                            return false
                        end,
                    }}\n",
                    symbol, regex_pattern, score
                );
                
                let path = format!("/etc/rspamd/lua.local.d/telegram_regex_{}.lua", symbol);
                
                let mut file = match OpenOptions::new().create(true).append(true).open(&path).await {
                    Ok(f) => f,
                    Err(e) => {
                        bot.send_message(chat_id, format!("Failed to open file: {e}")).await?;
                        return Ok(());
                    }
                };

                let _ = restart_rspamd_async();
                
                if let Err(e) = file.write_all(lua_rule.as_bytes()).await {
                    bot.send_message(chat_id, format!("Failed to write: {e}")).await?;
                    return Ok(());
                }
                
                bot.send_message(chat_id, format!(
                    "Added regex pattern: '{}' with symbol '{}' and score {}.\nPlease reload Rspamd to apply the rule.",
                    regex_pattern, symbol, score
                )).await?;
            }
            AdminCommand::Whitelist { pattern } => {
                
            }
            AdminCommand::Blacklist { pattern } => {
                
            }
        }
    } else {
        bot.send_message(chat_id, "You are not admin").await?;
    }
    Ok(())
}

async fn restart_rspamd_async() -> Result<()> {
    let output = Command::new("sudo")
        .arg("service")
        .arg("rspamd")
        .arg("restart")
        .output()                // runs and collects stdout/stderr
        .await?;

    if output.status.success() {
        println!("rspamd restarted successfully");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow::anyhow!("rspamd restart failed: {}", stderr))
    }
}
