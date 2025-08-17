use crate::admin_handlers::AdminCommand;
use crate::config::{field, key, suffix, ENABLED_FEATURES_KEY, reply_aware, rate_limit};
use crate::trust_manager::{TrustManager, TrustedMessageType, TrustedMessageMetadata};
use crate::bayes_manager::BayesManager;
use redis::{Commands, RedisResult};
use std::collections::HashMap;
use std::fmt::Write;
use teloxide::types::{Chat, ChatMemberStatus};
use teloxide::{prelude::*, types::InlineKeyboardButton, types::InlineKeyboardMarkup};
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use teloxide::types::{ChatId, MessageId, UserId};

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
                        format!("makeadmin:{}", chat),
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
                    /reputation <username> – show user's reputation\n\
                    /addregex <symbol|pattern|score> – add regex rule to rspamd\n\
                    /stats – show stats\n\
                    /whitelist <user|word>|<add|find>|<target>\n\
                    /blacklist <user|word>|<add|find>|<target>\n\
                    /marktrusted <message_id>|<bot|admin|verified> – mark message as trusted for reply-aware filtering\n\
                    /truststats – show trust management statistics\n\
                    \n\
                    Advanced Reply-Aware Filtering Commands:\n\
                    /replyconfig <setting>|<value> – configure reply-aware filtering settings\n\
                    /ratelimitstats – show rate limiting statistics\n\
                    /resetratelimit <user> – reset rate limiting for a user\n\
                    /spampatterns <user> – show spam pattern history for a user\n\
                    /selectivetrust <rule>|<true|false> – configure selective trusting rules\n\
                    /antievasionstats – show anti-evasion statistics\n\
                    \n\
                    Bayesian Learning Commands:\n\
                    /learnspam <message_id> – learn a message as spam for Bayesian classifier\n\
                    /learnham <message_id> – learn a message as ham for Bayesian classifier\n\
                    /bayesstats – show Bayesian classifier statistics\n\
                    /bayesreset – reset all Bayesian classifier data",
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
                            format!("stats:{}", chat),
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

            AdminCommand::MarkTrusted { args } => {
                // Parse args: "message_id|trust_type"
                let parts: Vec<&str> = args.split('|').map(str::trim).collect();
                if parts.len() != 2 {
                    bot.send_message(
                        chat_id,
                        "Usage: /marktrusted <message_id>|<bot|admin|verified>\n\
                     - message_id: the ID of the message to mark as trusted\n\
                     - trust_type: bot, admin, or verified",
                    )
                        .await?;
                    return Ok(());
                }

                let (message_id_str, trust_type_str) = (parts[0], parts[1]);
                
                // Parse message ID
                let message_id = match message_id_str.parse::<i32>() {
                    Ok(id) => MessageId(id),
                    Err(_) => {
                        bot.send_message(chat_id, "Invalid message ID. Must be a number.").await?;
                        return Ok(());
                    }
                };

                // Parse trust type
                let trust_type = match trust_type_str {
                    "bot" => TrustedMessageType::Bot,
                    "admin" => TrustedMessageType::Admin,
                    "verified" => TrustedMessageType::Verified,
                    _ => {
                        bot.send_message(
                            chat_id,
                            "Invalid trust type. Must be 'bot', 'admin', or 'verified'.",
                        )
                            .await?;
                        return Ok(());
                    }
                };

                // Initialize trust manager
                let trust_manager = match TrustManager::new("redis://127.0.0.1/") {
                    Ok(tm) => tm,
                    Err(e) => {
                        bot.send_message(
                            chat_id,
                            format!("Failed to initialize trust manager: {}", e),
                        )
                            .await?;
                        return Ok(());
                    }
                };

                // Create metadata for the trusted message
                let metadata = TrustedMessageMetadata::new(
                    message_id,
                    chat_id,
                    user_id,
                    trust_type.clone(),
                );

                // Mark the message as trusted
                match trust_manager.mark_trusted(metadata).await {
                    Ok(_) => {
                        bot.send_message(
                            chat_id,
                            format!(
                                "Message {} marked as trusted ({}) for reply-aware filtering.",
                                message_id.0,
                                trust_type.as_str()
                            ),
                        )
                            .await?;
                    }
                    Err(e) => {
                        bot.send_message(
                            chat_id,
                            format!("Failed to mark message as trusted: {}", e),
                        )
                            .await?;
                    }
                }
            }

            AdminCommand::TrustStats => {
                let trust_manager = TrustManager::new("redis://127.0.0.1/")
                    .expect("Failed to create trust manager");
                
                match trust_manager.get_stats().await {
                    Ok(stats) => {
                        bot.send_message(
                            chat_id,
                            format!(
                                "Trust Management Statistics:\n\
                                • Trusted messages: {}\n\
                                • Reply tracking entries: {}\n\
                                • Rate limiting enabled: {}\n\
                                • Anti-evasion enabled: {}\n\
                                • Selective trusting enabled: {}",
                                stats.trusted_messages,
                                stats.reply_tracking,
                                reply_aware::ENABLE_RATE_LIMITING,
                                reply_aware::ENABLE_ANTI_EVASION,
                                reply_aware::ENABLE_SELECTIVE_TRUSTING
                            ),
                        ).await?;
                    }
                    Err(e) => {
                        bot.send_message(
                            chat_id,
                            format!("Failed to get trust statistics: {}", e),
                        ).await?;
                    }
                }
            }
            
            AdminCommand::ReplyConfig { args } => {
                let parts: Vec<&str> = args.split('|').collect();
                if parts.len() != 2 {
                    bot.send_message(
                        chat_id,
                        "Usage: /replyconfig <setting>|<value>\n\
                        Settings: rate_limit, anti_evasion, selective_trust, max_reduction, min_spam_score",
                    ).await?;
                    return Ok(());
                }
                
                let setting = parts[0];
                let value = parts[1];
                
                match setting {
                    "rate_limit" => {
                        let enabled = value.parse::<bool>().unwrap_or(true);
                        bot.send_message(
                            chat_id,
                            format!("Rate limiting for reply-aware filtering: {}", enabled),
                        ).await?;
                    }
                    "anti_evasion" => {
                        let enabled = value.parse::<bool>().unwrap_or(true);
                        bot.send_message(
                            chat_id,
                            format!("Anti-evasion measures: {}", enabled),
                        ).await?;
                    }
                    "selective_trust" => {
                        let enabled = value.parse::<bool>().unwrap_or(true);
                        bot.send_message(
                            chat_id,
                            format!("Selective trusting: {}", enabled),
                        ).await?;
                    }
                    "max_reduction" => {
                        let reduction = value.parse::<f64>().unwrap_or(-5.0);
                        bot.send_message(
                            chat_id,
                            format!("Maximum score reduction: {}", reduction),
                        ).await?;
                    }
                    "min_spam_score" => {
                        let score = value.parse::<f64>().unwrap_or(1.0);
                        bot.send_message(
                            chat_id,
                            format!("Minimum spam score in replies: {}", score),
                        ).await?;
                    }
                    _ => {
                        bot.send_message(
                            chat_id,
                            "Invalid setting. Use: rate_limit, anti_evasion, selective_trust, max_reduction, min_spam_score",
                        ).await?;
                    }
                }
            }
            
            AdminCommand::RateLimitStats => {
                let mut conn = redis_client.get_connection().expect("Failed to get Redis connection");
                
                // Count rate limiting entries
                let trusted_rate_pattern = format!("{}*", rate_limit::TRUSTED_MESSAGE_RATE_PREFIX);
                let reply_rate_pattern = format!("{}*", rate_limit::REPLY_RATE_PREFIX);
                
                let trusted_rate_keys: Vec<String> = conn.keys(&trusted_rate_pattern).unwrap_or_default();
                let reply_rate_keys: Vec<String> = conn.keys(&reply_rate_pattern).unwrap_or_default();
                
                let mut trusted_rate_count = 0;
                let mut reply_rate_count = 0;
                
                for key in &trusted_rate_keys {
                    let count: u32 = conn.get(key).unwrap_or(0);
                    trusted_rate_count += count;
                }
                
                for key in &reply_rate_keys {
                    let count: u32 = conn.get(key).unwrap_or(0);
                    reply_rate_count += count;
                }
                
                bot.send_message(
                    chat_id,
                    format!(
                        "Rate Limiting Statistics:\n\
                        • Trusted message rate limits: {} ({} total)\n\
                        • Reply rate limits: {} ({} total)\n\
                        • Max trusted messages per hour: {}\n\
                        • Max replies per hour: {}",
                        trusted_rate_keys.len(),
                        trusted_rate_count,
                        reply_rate_keys.len(),
                        reply_rate_count,
                        reply_aware::MAX_TRUSTED_MESSAGES_PER_HOUR,
                        reply_aware::MAX_REPLIES_PER_HOUR
                    ),
                ).await?;
            }
            
            AdminCommand::ResetRateLimit { user } => {
                let mut conn = redis_client.get_connection().expect("Failed to get Redis connection");
                
                let trusted_rate_key = format!("{}{}", rate_limit::TRUSTED_MESSAGE_RATE_PREFIX, user);
                let reply_rate_key = format!("{}{}", rate_limit::REPLY_RATE_PREFIX, user);
                
                let _: () = conn.del(&trusted_rate_key).unwrap_or(());
                let _: () = conn.del(&reply_rate_key).unwrap_or(());
                
                bot.send_message(
                    chat_id,
                    format!("Rate limiting reset for user: {}", user),
                ).await?;
            }
            
            AdminCommand::SpamPatterns { user } => {
                let trust_manager = TrustManager::new("redis://127.0.0.1/")
                    .expect("Failed to create trust manager");
                
                let user_id = user.parse::<u64>().unwrap_or(0);
                match trust_manager.get_spam_patterns(UserId(user_id)).await {
                    Ok(patterns) => {
                        if patterns.is_empty() {
                            bot.send_message(
                                chat_id,
                                format!("No spam patterns found for user: {}", user),
                            ).await?;
                        } else {
                            bot.send_message(
                                chat_id,
                                format!(
                                    "Spam patterns for user {}:\n{}",
                                    user,
                                    patterns.join("\n")
                                ),
                            ).await?;
                        }
                    }
                    Err(e) => {
                        bot.send_message(
                            chat_id,
                            format!("Failed to get spam patterns: {}", e),
                        ).await?;
                    }
                }
            }
            
            AdminCommand::SelectiveTrust { args } => {
                let parts: Vec<&str> = args.split('|').collect();
                if parts.len() != 2 {
                    bot.send_message(
                        chat_id,
                        "Usage: /selectivetrust <rule>|<true|false>\n\
                        Rules: trust_bot, trust_admin, trust_verified, trust_good_reputation, trust_recent_only",
                    ).await?;
                    return Ok(());
                }
                
                let rule = parts[0];
                let enabled = parts[1].parse::<bool>().unwrap_or(true);
                
                match rule {
                    "trust_bot" => {
                        bot.send_message(
                            chat_id,
                            format!("Trust bot messages: {}", enabled),
                        ).await?;
                    }
                    "trust_admin" => {
                        bot.send_message(
                            chat_id,
                            format!("Trust admin messages: {}", enabled),
                        ).await?;
                    }
                    "trust_verified" => {
                        bot.send_message(
                            chat_id,
                            format!("Trust verified user messages: {}", enabled),
                        ).await?;
                    }
                    "trust_good_reputation" => {
                        bot.send_message(
                            chat_id,
                            format!("Trust messages from users with good reputation: {}", enabled),
                        ).await?;
                    }
                    "trust_recent_only" => {
                        bot.send_message(
                            chat_id,
                            format!("Trust only recent messages: {}", enabled),
                        ).await?;
                    }
                    _ => {
                        bot.send_message(
                            chat_id,
                            "Invalid rule. Use: trust_bot, trust_admin, trust_verified, trust_good_reputation, trust_recent_only",
                        ).await?;
                    }
                }
            }
            
            AdminCommand::AntiEvasionStats => {
                let mut conn = redis_client.get_connection().expect("Failed to get Redis connection");
                
                // Count spam pattern entries
                let spam_pattern_prefix = format!("{}*", rate_limit::SPAM_PATTERN_PREFIX);
                let spam_pattern_keys: Vec<String> = conn.keys(&spam_pattern_prefix).unwrap_or_default();
                
                let mut total_patterns = 0;
                for key in &spam_pattern_keys {
                    let patterns: Vec<String> = conn.smembers(key).unwrap_or_default();
                    total_patterns += patterns.len();
                }
                
                bot.send_message(
                    chat_id,
                    format!(
                        "Anti-Evasion Statistics:\n\
                        • Users with spam patterns: {}\n\
                        • Total spam patterns tracked: {}\n\
                        • Max links in reply: {}\n\
                        • Max phone numbers in reply: {}\n\
                        • Max invite links in reply: {}\n\
                        • Max caps ratio in reply: {}\n\
                        • Max emoji in reply: {}",
                        spam_pattern_keys.len(),
                        total_patterns,
                        reply_aware::anti_evasion::MAX_LINKS_IN_REPLY,
                        reply_aware::anti_evasion::MAX_PHONE_NUMBERS_IN_REPLY,
                        reply_aware::anti_evasion::MAX_INVITE_LINKS_IN_REPLY,
                        reply_aware::anti_evasion::MAX_CAPS_RATIO_IN_REPLY,
                        reply_aware::anti_evasion::MAX_EMOJI_IN_REPLY
                    ),
                ).await?;
            }

            AdminCommand::LearnSpam { message_id } => {
                let bayes_manager = match BayesManager::new() {
                    Ok(manager) => manager,
                    Err(e) => {
                        bot.send_message(
                            chat_id,
                            format!("❌ Failed to create Bayes manager: {}", e)
                        ).await?;
                        return Ok(());
                    }
                };
                
                // Get message content from Redis
                let content = match get_message_content(&mut redis_conn, &message_id).await {
                    Ok(content) => content,
                    Err(e) => {
                        bot.send_message(
                            chat_id,
                            format!("❌ Failed to get message content: {}", e)
                        ).await?;
                        return Ok(());
                    }
                };
                
                match bayes_manager.learn_spam(&message_id, &content).await {
                    Ok(()) => {
                        bot.send_message(
                            chat_id,
                            format!("✅ Message {} learned as spam", message_id)
                        ).await?;
                    }
                    Err(e) => {
                        bot.send_message(
                            chat_id,
                            format!("❌ Failed to learn as spam: {}", e)
                        ).await?;
                    }
                }
            }
            
            AdminCommand::LearnHam { message_id } => {
                let bayes_manager = match BayesManager::new() {
                    Ok(manager) => manager,
                    Err(e) => {
                        bot.send_message(
                            chat_id,
                            format!("❌ Failed to create Bayes manager: {}", e)
                        ).await?;
                        return Ok(());
                    }
                };
                
                let content = match get_message_content(&mut redis_conn, &message_id).await {
                    Ok(content) => content,
                    Err(e) => {
                        bot.send_message(
                            chat_id,
                            format!("❌ Failed to get message content: {}", e)
                        ).await?;
                        return Ok(());
                    }
                };
                
                match bayes_manager.learn_ham(&message_id, &content).await {
                    Ok(()) => {
                        bot.send_message(
                            chat_id,
                            format!("✅ Message {} learned as ham", message_id)
                        ).await?;
                    }
                    Err(e) => {
                        bot.send_message(
                            chat_id,
                            format!("❌ Failed to learn as ham: {}", e)
                        ).await?;
                    }
                }
            }
            
            AdminCommand::BayesStats => {
                let bayes_manager = match BayesManager::new() {
                    Ok(manager) => manager,
                    Err(e) => {
                        bot.send_message(
                            chat_id,
                            format!("❌ Failed to create Bayes manager: {}", e)
                        ).await?;
                        return Ok(());
                    }
                };
                
                match bayes_manager.get_detailed_info() {
                    Ok(info) => {
                        let status = if info.get("is_ready").unwrap_or(&"false".to_string()) == "true" { 
                            "✅ Ready" 
                        } else { 
                            "⏳ Training" 
                        };
                        
                        let response = format!(
                            "🤖 Bayes Classifier Status: {}\n\n\
                             📊 Statistics:\n\
                             • Spam tokens: {}\n\
                             • Ham tokens: {}\n\
                             • Spam messages: {}\n\
                             • Ham messages: {}\n\
                             • Total messages: {}\n\
                             • Spam ratio: {}%\n\n\
                             📈 Progress:\n\
                             • Spam progress: {}%\n\
                             • Ham progress: {}%\n\
                             • Min required: {} spam, {} ham",
                            status,
                            info.get("spam_tokens").unwrap_or(&"0".to_string()),
                            info.get("ham_tokens").unwrap_or(&"0".to_string()),
                            info.get("spam_messages").unwrap_or(&"0".to_string()),
                            info.get("ham_messages").unwrap_or(&"0".to_string()),
                            info.get("total_messages").unwrap_or(&"0".to_string()),
                            info.get("spam_ratio_percent").unwrap_or(&"0".to_string()),
                            info.get("spam_progress_percent").unwrap_or(&"0".to_string()),
                            info.get("ham_progress_percent").unwrap_or(&"0".to_string()),
                            info.get("min_spam_required").unwrap_or(&"200".to_string()),
                            info.get("min_ham_required").unwrap_or(&"200".to_string())
                        );
                        
                        bot.send_message(chat_id, response).await?;
                    }
                    Err(e) => {
                        bot.send_message(
                            chat_id,
                            format!("❌ Failed to get Bayes stats: {}", e)
                        ).await?;
                    }
                }
            }
            
            AdminCommand::BayesReset => {
                let bayes_manager = match BayesManager::new() {
                    Ok(manager) => manager,
                    Err(e) => {
                        bot.send_message(
                            chat_id,
                            format!("❌ Failed to create Bayes manager: {}", e)
                        ).await?;
                        return Ok(());
                    }
                };
                
                match bayes_manager.reset_all_data() {
                    Ok(()) => {
                        bot.send_message(
                            chat_id,
                            "🗑️ Bayes classifier data has been reset. The classifier will need to be retrained."
                        ).await?;
                    }
                    Err(e) => {
                        bot.send_message(
                            chat_id,
                            format!("❌ Failed to reset Bayes data: {}", e)
                        ).await?;
                    }
                }
            }

        }
    } else {
        bot.send_message(chat_id, "You are not admin").await?;
    }
    Ok(())
}

/// Retrieves message content from Redis storage.
/// 
/// # Arguments
/// 
/// * `redis_conn` - Mutable Redis connection
/// * `message_id` - The message ID to retrieve content for
/// 
/// # Returns
/// 
/// A `Result<String>` containing the message content or an error
async fn get_message_content(redis_conn: &mut redis::Connection, message_id: &str) -> Result<String> {
    // Try to get message content from Redis
    let key = format!("tg:message:{}", message_id);
    let content: Option<String> = redis_conn.get(&key)?;
    
    if let Some(content) = content {
        Ok(content)
    } else {
        // If not found in Redis, return a placeholder message
        // In a real implementation, you might want to fetch from Telegram API
        Ok(format!("Message content not found for ID: {}", message_id))
    }
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
