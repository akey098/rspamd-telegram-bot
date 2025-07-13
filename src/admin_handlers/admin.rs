use crate::admin_handlers::AdminCommand;
use redis::{Commands, RedisResult};
use std::collections::HashMap;
use std::fmt::Write;
use teloxide::types::{Chat, ChatMemberStatus};
use teloxide::{prelude::*, types::InlineKeyboardButton, types::InlineKeyboardMarkup};
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use crate::config::{field, key, suffix, ENABLED_FEATURES_KEY};

use anyhow::Result;
use regex::Regex;

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

/// Shared helper for both whitelist and blacklist logic.
///
/// - `bot` / `chat_id`: for sending replies.
/// - `redis_conn`: mutable connection to Redis.
/// - `redis_key`: the exact SET key (e.g. `key::TG_WHITELIST_USER_KEY`).
/// - `item_kind`: `"user"` or `"word"` (used in reply text).
/// - `list_name`: `"whitelist"` or `"blacklist"` (used in reply text).
/// - `action`: must be `"add"` or `"find"`.
/// - `target`: the third part of the pattern. If `action=="add"`, it must not be `"*"`.
///             If `action=="find"`, it can be `"*"`, a plain literal, or a Rust‐regex.
///
/// This sends the appropriate reply and returns `Ok(())`.
async fn process_set(
    bot: &Bot,
    chat_id: ChatId,
    redis_conn: &mut redis::Connection,
    redis_key: &str,
    item_kind: &str,  // "user" or "word"
    list_name: &str,  // "whitelist" or "blacklist"
    action: &str,     // "add" or "find"
    target: &str,     // third part of the pattern
) -> ResponseResult<()> {
    match action {
        // ────────────────────────────────────────────────────────────────────
        "add" => {
            // You cannot do “add|*”. Must specify exactly one literal (user_id or word).
            if target == "*" {
                bot.send_message(
                    chat_id,
                    format!(
                        "Cannot use `*` with `add`. You must specify exactly one {} to add.",
                        item_kind
                    ),
                )
                    .await?;
            } else {
                // Straight SADD
                let rv: RedisResult<()> = redis_conn.sadd(redis_key, target);
                match rv {
                    Ok(()) => {
                        bot.send_message(
                            chat_id,
                            format!("Added {} `{}` to the {}.", item_kind, target, list_name),
                        )
                            .await?;
                    }
                    Err(e) => {
                        bot.send_message(
                            chat_id,
                            format!("Failed to add {} to {}: {}", item_kind, target, e),
                        )
                            .await?;
                    }
                }
            }
        }

        // ────────────────────────────────────────────────────────────────────
        "find" => {
            // 1) If target == "*", list all members via SMEMBERS.
            if target == "*" {
                let all_items: Vec<String> =
                    redis_conn.smembers(redis_key).unwrap_or_else(|_| Vec::new());
                if all_items.is_empty() {
                    bot.send_message(chat_id, format!("(no {}ed {}s)", list_name, item_kind))
                        .await?;
                } else {
                    let joined = all_items.join(", ");
                    bot.send_message(
                        chat_id,
                        format!("{}ed {}s: {}",
                                // Capitalize first letter of list_name for nicer output
                                {
                                    let mut s = list_name.to_owned();
                                    s.get_mut(0..1).map(|c| c.make_ascii_uppercase());
                                    s
                                },
                                item_kind,
                                joined,
                        ),
                    )
                        .await?;
                }
                return Ok(());
            }

            // 2) If no regex meta‐characters, treat target as a plain literal → SISMEMBER
            let is_plain_literal = !target.chars().any(|c| {
                matches!(
                    c,
                    '.' | '^' | '$' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '\\'
                )
            });

            if is_plain_literal {
                let exists: bool = redis_conn.sismember(redis_key, target).unwrap_or(false);
                if exists {
                    bot.send_message(
                        chat_id,
                        format!(
                            "{} `{}` is in the {}.",
                            item_kind, target, list_name
                        ),
                    )
                        .await?;
                } else {
                    bot.send_message(
                        chat_id,
                        format!(
                            "{} `{}` is NOT in the {}.",
                            item_kind, target, list_name
                        ),
                    )
                        .await?;
                }
            } else {
                // 3) Regex case: attempt to compile `target` as a Rust regex,
                // then SMEMBERS and filter in Rust.
                let pattern = match Regex::new(target) {
                    Ok(rgx) => rgx,
                    Err(e) => {
                        bot.send_message(
                            chat_id,
                            format!("Invalid regex `{}`: {}", target, e),
                        )
                            .await?;
                        return Ok(());
                    }
                };

                let all_items: Vec<String> =
                    redis_conn.smembers(redis_key).unwrap_or_else(|_| Vec::new());
                let mut matches = Vec::new();
                for item in all_items.iter() {
                    if pattern.is_match(item) {
                        matches.push(item.clone());
                    }
                }

                if matches.is_empty() {
                    bot.send_message(
                        chat_id,
                        format!(
                            "No {}ed {}s match `/ {}`.",
                            list_name, item_kind, target
                        ),
                    )
                        .await?;
                } else {
                    let joined = matches.join(", ");
                    bot.send_message(
                        chat_id,
                        format!(
                            "{}s matching `/ {}`: {}",
                            // Capitalize list_name for output
                            {
                                let mut s = list_name.to_owned();
                                s.get_mut(0..1).map(|c| c.make_ascii_uppercase());
                                s
                            },
                            target,
                            joined
                        ),
                    )
                        .await?;
                }
            }
        }

        _ => {
            // Should never happen if the caller only passes "add" or "find"
            bot.send_message(
                chat_id,
                format!("Invalid action `{}`. Must be `add` or `find`.", action),
            )
                .await?;
        }
    }

    Ok(())
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
                    "Commands:\n\
                    /help – show help for commands\n\
                    /makeadmin – register current chat as admin control chat\n\
                    /reputation <username> – show user’s reputation\n\
                    /addregex <symbol|pattern|score> – add regex rule to rspamd\n\
                    /stats – show stats\n\
                    /whitelist <user|word>|<add|find>|<target>\n\
                    /blacklist <user|word>|<add|find>|<target>",
                ).await?;
            }
            AdminCommand::ManageFeatures => {
                let redis_client =
                    redis::Client::open("redis://127.0.0.1/").expect("Failed to connect to Redis");
                let mut redis_conn = redis_client
                    .get_connection()
                    .expect("Failed to get Redis connection");

                let key_moderated =
                    format!("{}{}{}", key::ADMIN_PREFIX, chat_id.0, suffix::MODERATED_CHATS);
                let moderated_chats: Vec<i64> =
                    redis_conn.smembers(key_moderated).unwrap_or_else(|_| Vec::new());

                // 2) Build one row-per-chat button: callback_data = "managefeat:<chat_id>"
                let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();
                for chat in moderated_chats.into_iter() {
                    // You might want to fetch the chat's stored name for labeling:
                    let chat_name: String = redis_conn
                        .hget(format!("{}{}", key::TG_CHATS_PREFIX, chat), field::NAME)
                        .unwrap_or_else(|_| chat.to_string());
                    rows.push(vec![
                        InlineKeyboardButton::callback(
                            format!("Chat: {}", chat_name),
                            format!("managefeat:{}", chat),
                        ),
                    ]);
                }
                // If the admin has no moderated chats, send a simple message instead:
                if rows.is_empty() {
                    bot.send_message(chat_id, "You do not moderate any chats yet.")
                        .await?;
                } else {
                    let keyboard = InlineKeyboardMarkup::new(rows);
                    bot.send_message(chat_id, "Select a chat to manage features:")
                        .reply_markup(keyboard)
                        .await?;
                }
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

                // Register the new symbol as a feature enabled by default
                let _: redis::RedisResult<()> = redis_conn.sadd(ENABLED_FEATURES_KEY, symbol);

                bot.send_message(chat_id, format!(
                    "Added regex pattern: '{}' with symbol '{}' and score {}.\nPlease reload Rspamd to apply the rule.",
                    regex_pattern, symbol, score
                )).await?;
            }
            AdminCommand::Whitelist { pattern } => {
                // Now expect exactly 3 parts: kind|action|target
                let parts: Vec<&str> = pattern.split('|').map(str::trim).collect();
                if parts.len() != 3 {
                    bot.send_message(
                        chat_id,
                        "Usage: /whitelist <user|word>|<add|find>|<target>\n\
                     - If add: target must be the literal user_id or word.\n\
                     - If find: target can be '*' (list all),\n\
                       or a plain string (SISMEMBER),\n\
                       or a Rust‐regex (full regex syntax).",
                    )
                        .await?;
                    return Ok(());
                }

                let (kind, action, target) = (parts[0], parts[1], parts[2]);

                // Dispatch to the shared helper with the correct Redis key
                match kind {
                    "user" => {
                        process_set(
                            &bot,
                            chat_id,
                            &mut redis_conn,
                            key::TG_WHITELIST_USER_KEY,
                            "user",
                            "whitelist",
                            action,
                            target,
                        )
                            .await?;
                    }
                    "word" => {
                        process_set(
                            &bot,
                            chat_id,
                            &mut redis_conn,
                            key::TG_WHITELIST_WORD_KEY,
                            "word",
                            "whitelist",
                            action,
                            target,
                        )
                            .await?;
                    }
                    _ => {
                        bot.send_message(
                            chat_id,
                            "First part must be `user` or `word`. Usage: /whitelist <user|word>|<add|find>|<target>",
                        )
                            .await?;
                    }
                }
            }

            AdminCommand::Blacklist { pattern } => {
                // Exactly the same parsing, but pass in the BLACKLIST key
                let parts: Vec<&str> = pattern.split('|').map(str::trim).collect();
                if parts.len() != 3 {
                    bot.send_message(
                        chat_id,
                        "Usage: /blacklist <user|word>|<add|find>|<target>\n\
                     - If add: target must be the literal user_id or word.\n\
                     - If find: target can be '*' (list all),\n\
                       or a plain string (SISMEMBER),\n\
                       or a Rust‐regex (full regex syntax).",
                    )
                        .await?;
                    return Ok(());
                }

                let (kind, action, target) = (parts[0], parts[1], parts[2]);

                match kind {
                    "user" => {
                        process_set(
                            &bot,
                            chat_id,
                            &mut redis_conn,
                            key::TG_BLACKLIST_USER_KEY,
                            "user",
                            "blacklist",
                            action,
                            target,
                        )
                            .await?;
                    }
                    "word" => {
                        process_set(
                            &bot,
                            chat_id,
                            &mut redis_conn,
                            key::TG_BLACKLIST_WORD_KEY,
                            "word",
                            "blacklist",
                            action,
                            target,
                        )
                            .await?;
                    }
                    _ => {
                        bot.send_message(
                            chat_id,
                            "First part must be `user` or `word`. Usage: /blacklist <user|word>|<add|find>|<target>",
                        )
                            .await?;
                    }
                }
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
