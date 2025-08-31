use std::collections::HashMap;
use crate::admin_handlers::{handle_admin_command, AdminCommand};
use crate::handlers::handle_message;
use redis::{Commands, RedisResult};
use teloxide::dispatching::{Dispatcher, UpdateFilterExt};
use teloxide::dptree;
use teloxide::payloads::{AnswerCallbackQuerySetters, SendMessageSetters, SetMyCommandsSetters};
use teloxide::prelude::{CallbackQuery, ChatId, ChatMemberUpdated, Message, Requester, Update};
use teloxide::types::{BotCommand, BotCommandScope, ChatKind, ChatMemberStatus, InlineKeyboardButton, InlineKeyboardMarkup};
use teloxide::utils::command::BotCommands;
use teloxide::{Bot, RequestError};
use std::fmt::Write;
use crate::config::{field, key, suffix, DEFAULT_FEATURES, ENABLED_FEATURES_KEY};

/// Helper function to parse commands that may have bot username appended
fn parse_command_with_botname<T: teloxide::utils::command::BotCommands>(text: &str, bot_name: &str) -> Result<T, teloxide::utils::command::ParseError> {
    T::parse(text, bot_name).or_else(|_| {
        // If parsing fails, try to extract command without bot username
        if text.starts_with('/') {
            let parts: Vec<&str> = text.splitn(2, '@').collect();
            if parts.len() == 2 {
                // Command has @botname format, try parsing just the command part
                T::parse(parts[0], bot_name)
            } else {
                // No @ found, return original error
                T::parse(text, bot_name)
            }
        } else {
            // Not a command, return original error
            T::parse(text, bot_name)
        }
    })
}

pub async fn message_handler(bot: Bot, msg: Message) -> Result<(), RequestError> {
    if let Some(text) = msg.text() {
        let client = redis::Client::open("redis://127.0.0.1/").expect("failed to get redis client.");
        let mut conn = client.get_connection().expect("Failed to connect");
        let key = format!("{}{}", key::TG_USERS_PREFIX, msg.clone().from.unwrap().id.0);

        let user_rep: RedisResult<i64> = conn.hget(&key, field::REP);
        
        match user_rep {
            Ok(_) => {}
            Err(_) => {
                let _: () = conn
                    .hset(key.clone(), field::REP, 0)
                    .expect("Failed to update user's reputation");
                let _: () = conn
                    .hset(key.clone(), field::USERNAME, msg.clone().from.unwrap().username.unwrap().to_string())
                    .expect("Failed to update user's reputation");
            }
        }
        
        // Try to parse as admin command, handling both formats:
        // 1. /command (in private chats)
        // 2. /command@botname (in group chats)
        let cmd_result = parse_command_with_botname::<AdminCommand>(text, "rspamd-bot");
        
        if let Ok(cmd) = cmd_result {
            handle_admin_command(bot.clone(), msg.clone(), cmd).await?;
        } else {
            let _ = handle_message(bot.clone(), msg.clone()).await;
        }
    }
    Ok(())
}


pub async fn makeadmin_handler(bot: Bot, query: CallbackQuery) -> Result<(), RequestError> {
    if let Some(callback_data) = query.data {
        if let Some(admin_chat) = query.message {
            let admin_id = admin_chat.chat().id;
            let selected_chat: i64 = callback_data["makeadmin:".len()..].parse().unwrap();
            if selected_chat != 0 {
                let redis_client =
                    redis::Client::open("redis://127.0.0.1/").expect("Failed to connect to Redis");
                let mut redis_conn = redis_client
                    .get_connection()
                    .expect("Failed to get Redis connection");
                let key = format!("{}{}{}", key::ADMIN_PREFIX, admin_id, suffix::MODERATED_CHATS);
                let _: () = redis_conn
                    .sadd(key, selected_chat.clone())
                    .expect("Failed to add moderated chat to admin");
                
                let _: () = redis_conn
                    .hset(format!("{}{}", key::TG_CHATS_PREFIX, selected_chat.clone()), field::ADMIN_CHAT, admin_id.0)
                    .expect("Failed to add admin chat to selected chat");

                bot.answer_callback_query(query.id)
                    .text("Chat assigned for moderation!")
                    .await?;
            }
        }
    }
    Ok(())
}

pub async fn stats_handler(bot: Bot, query: CallbackQuery) -> Result<(), RequestError> {
    if let Some(callback_data) = query.data {
        if let Some(admin_chat) = query.message {
            let admin_id = admin_chat.chat().id;
            let selected_chat: i64 = callback_data["stats:".len()..].parse().unwrap();
            let redis_client =
                redis::Client::open("redis://127.0.0.1/").expect("Failed to connect to Redis");
            let mut redis_conn = redis_client
                .get_connection()
                .expect("Failed to get Redis connection");
            
            // Get the chat name for the response
            let chat_name: String = redis_conn
                .hget(format!("{}{}", key::TG_CHATS_PREFIX, selected_chat), field::NAME)
                .unwrap_or_else(|_| selected_chat.to_string());
            
            let stats: HashMap<String, String> = redis_conn
                .hgetall(format!("{}{}", key::TG_CHATS_PREFIX, selected_chat))
                .expect("Failed to get chat stats");
            let mut response = String::new();
            writeln!(&mut response, "Stats for chat: {}", chat_name).unwrap();
            for (field, value) in stats {
                if field == field::NAME || field == field::ADMIN_CHAT {
                    continue;
                }
                writeln!(&mut response, "{}: {}", field, value).unwrap();
            }
            bot.send_message(admin_id, response).await?;
        }
    }
    Ok(())
}


/// 1) Called when callback_data starts with "managefeat:"
/// i.e. admin clicked “Chat: XYZ”
/// We now show a second inline keyboard listing each feature for that chat,
/// along with a “Discard” button.
pub async fn manage_features_select_chat(
    bot: Bot,
    query: CallbackQuery,
) -> Result<(), RequestError> {
    // Make sure there is callback data and a message
    if let Some(data) = query.data {
        if let Some(callback_msg) = query.message {
            // Acknowledge the callback (so Telegram's “loading” spinner stops)
            let _ = bot
                .answer_callback_query(query.id.clone())
                .text("Fetching features…")
                .await;

            // Extract chat_id that the admin clicked
            // format is "managefeat:<chat_id>"
            let chat_id_raw = &data["managefeat:".len()..];
            if let Ok(target_chat_id) = chat_id_raw.parse::<i64>() {
                let redis_client =
                    redis::Client::open("redis://127.0.0.1/").expect("Failed to connect to Redis");
                let mut redis_conn = redis_client
                    .get_connection()
                    .expect("Failed to get Redis connection");

                let chat_key = format!("{}{}", key::TG_CHATS_PREFIX, target_chat_id);
                let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();

                // Show all available features, not just the ones already in Redis
                let feats: Vec<String> = DEFAULT_FEATURES
                    .iter()
                    .map(|&s| s.to_string())
                    .collect();

                for feat in feats {
                    let field_name = format!("feat:{}", feat);
                    let chat_val: Option<String> =
                        redis_conn.hget(chat_key.clone(), field_name.clone()).unwrap_or(None);
                    let global_on: bool =
                        redis_conn.sismember(ENABLED_FEATURES_KEY, feat.clone()).unwrap_or(false);
                    let is_enabled = match chat_val.as_deref() {
                        Some("1") => true,
                        Some("0") => false,
                        _ => global_on,
                    };

                    // Decide button text and callback_data:
                    if is_enabled {
                        // If currently enabled → we want a “Disable” button
                        rows.push(vec![InlineKeyboardButton::callback(
                            format!("Disable `{}`", feat),
                            format!("togglefeat:{}|{}", target_chat_id, feat),
                        )]);
                    } else {
                        // If currently disabled → we want an “Enable” button
                        rows.push(vec![InlineKeyboardButton::callback(
                            format!("Enable `{}`", feat),
                            format!("togglefeat:{}|{}", target_chat_id, feat),
                        )]);
                    }
                }

                // 3) Finally, add one more row with “Discard” to just delete the message
                rows.push(vec![InlineKeyboardButton::callback(
                    "Discard".to_string(),
                    format!("discard:{}", target_chat_id),
                )]);

                let keyboard = InlineKeyboardMarkup::new(rows);

                // 4) Instead of sending a brand‐new message, we can edit the old one OR simply send
                //    a new message. To keep things simple, we’ll delete the old inline‐keyboard message,
                //    then send a new one. (Either approach is fine; Telegram will let you delete it.)
                let old_chat = callback_msg.chat().id;
                let old_msg_id = callback_msg.id();
                let _ = bot.delete_message(old_chat, old_msg_id).await;

                bot.send_message(old_chat, "Select a feature to toggle:")
                    .reply_markup(keyboard)
                    .await?;
            }
        }
    }
    Ok(())
}

/// 2) Called when callback_data starts with "togglefeat:"
/// That means “toggle feature <feat> for chat <chat_id>”
/// We flip the HSET/HDEL in Redis, then delete the inline‐keyboard message
pub async fn toggle_feature_handler(
    bot: Bot,
    query: CallbackQuery,
) -> Result<(), RequestError> {
    if let Some(data) = query.data {
        if let Some(callback_msg) = query.message {
            // Acknowledge quickly
            let _ = bot
                .answer_callback_query(query.id.clone())
                .await;

            // data = "togglefeat:<chat_id>|<feat_name>"
            let rest = &data["togglefeat:".len()..];
            // rest = "<chat_id>|<feat_name>"
            if let Some((chat_id_raw, feat_name)) = rest.split_once('|') {
                if let Ok(target_chat_id) = chat_id_raw.parse::<i64>() {
                    let redis_client =
                        redis::Client::open("redis://127.0.0.1/").expect("Failed to connect to Redis");
                    let mut redis_conn = redis_client
                        .get_connection()
                        .expect("Failed to get Redis connection");
                    let chat_key = format!("{}{}", key::TG_CHATS_PREFIX, target_chat_id);
                    let field_name = format!("feat:{}", feat_name);

                    let chat_val: Option<String> =
                        redis_conn.hget(chat_key.clone(), field_name.clone()).unwrap_or(None);
                    let global_on: bool =
                        redis_conn.sismember(ENABLED_FEATURES_KEY, feat_name).unwrap_or(false);
                    let currently_on = match chat_val.as_deref() {
                        Some("1") => true,
                        Some("0") => false,
                        _ => global_on,
                    };

                    if currently_on {
                        // It was on → set explicit 0 (disable)
                        let _: () = redis_conn
                            .hset(chat_key.clone(), field_name.clone(), "0")
                            .expect("Failed to disable feature");
                        // Optionally send ephemeral feedback:
                        let _ = bot
                            .send_message(
                                callback_msg.chat().id,
                                format!("Feature `{}` disabled for chat {}", feat_name, target_chat_id),
                            )
                            .await;
                    } else {
                        // It was off → set it to "1" (enable)
                        let _:() = redis_conn.hset(chat_key.clone(), field_name.clone(), "1").expect("Failed to set feature");
                        let _ = bot
                            .send_message(
                                callback_msg.chat().id,
                                format!("Feature `{}` enabled for chat {}", feat_name, target_chat_id),
                            )
                            .await;
                    }

                    // Finally, delete the inline‐keyboard message itself
                    let old_chat = callback_msg.chat().id;
                    let old_msg_id = callback_msg.id();
                    let _ = bot.delete_message(old_chat, old_msg_id).await;
                }
            }
        }
    }
    Ok(())
}

/// 3) Called when callback_data starts with "discard:"
/// We simply delete the inline‐keyboard message, without changes
pub async fn discard_handler(
    bot: Bot,
    query: CallbackQuery,
) -> Result<(), RequestError> {
    if let Some(_data) = query.data {
        if let Some(callback_msg) = query.message {
            // Acknowledge
            let _ = bot.answer_callback_query(query.id.clone()).await;

            // data = "discard:<chat_id>", but we don’t actually need the chat_id here.
            // We just delete the message containing the inline keyboard:
            let old_chat = callback_msg.chat().id;
            let old_msg_id = callback_msg.id();
            let _ = bot.delete_message(old_chat, old_msg_id).await;
        }
    }
    Ok(())
}

pub async fn chat_member_handler(
    _bot: Bot,
    update: ChatMemberUpdated,
) -> Result<(), RequestError> {
    let new_status = update.new_chat_member.status();
    let chat_id = ChatId(update.chat.id.0);
    let username = update.new_chat_member.user.username.as_ref().unwrap();

    let client = redis::Client::open("redis://127.0.0.1/").expect("failed to get redis client.");
    let mut conn = client.get_connection().expect("Failed to connect");
    let key = format!("{}{}", key::TG_USERS_PREFIX, update.new_chat_member.user.id.0);
    let admin_key = format!("{}{}", update.new_chat_member.user.id, suffix::BOT_CHATS);

    match new_status {
        ChatMemberStatus::Member | ChatMemberStatus::Administrator | ChatMemberStatus::Owner => {
            if new_status == ChatMemberStatus::Administrator || new_status == ChatMemberStatus::Owner {
                if !update.new_chat_member.user.is_bot {
                    let _: () = conn
                        .sadd(admin_key.clone(), chat_id.0)
                        .expect("Failed to add chat to admin's bot_chats");
                }
            }
            let _: () = conn.hset(key.clone(), field::REP, 0)
                .expect("Failed to set rep");

            let _: () = conn.hset(key.clone(), field::USERNAME, &username)
                .expect("Failed to set username");
        }
        ChatMemberStatus::Left | ChatMemberStatus::Banned | ChatMemberStatus::Restricted => {
            if update.old_chat_member.status() == ChatMemberStatus::Administrator || update.old_chat_member.status() == ChatMemberStatus::Owner {
                if !update.new_chat_member.user.is_bot {
                    let _: () = conn
                        .srem(admin_key.clone(), chat_id.0)
                        .expect("Failed to remove chat from bot_chats");
                }
            }
            let _: () = conn
                .del(key.clone())
                .expect("Failed to remove user's reputation");
        }
    }

    Ok(())
}

pub async fn my_chat_member_handler(
    bot: Bot,
    update: ChatMemberUpdated,
) -> Result<(), RequestError> {
    if matches!(update.chat.kind, ChatKind::Private { .. }) {
        return Ok(());
    }
    let client = redis::Client::open("redis://127.0.0.1/").expect("failed to get redis client.");
    let mut conn = client.get_connection().expect("Failed to connect");
    let chat_id = ChatId(update.chat.id.0);
    let admins_key = format!("{}{}", update.chat.id.0, suffix::ADMINS);
    let chat_key = format!("{}{}", key::TG_CHATS_PREFIX, update.chat.id.0);
    if update.new_chat_member.status() == ChatMemberStatus::Banned || update.new_chat_member.status() == ChatMemberStatus::Left || update.new_chat_member.status() == ChatMemberStatus::Restricted {
        let admins: Vec<String> = conn
            .smembers(admins_key.clone())
            .expect("failed to get admins of the chat");
        for admin in admins {
            let admin_key = format!("{}{}", admin, suffix::BOT_CHATS);
            let _: () = conn
                .srem(admin_key, update.chat.id.0)
                .expect("Failed to remove admin from bot_chats");
        }
        let _: () = conn
            .del(chat_key)
            .expect("Failed to delete chat");
    } else {
        let _: () = conn
            .hset(&chat_key, field::NAME, update.chat.title().unwrap())
            .expect("Failed to set up chat key");
        // initialize all features as enabled by default for the chat
        for feat in DEFAULT_FEATURES {
            let field = format!("feat:{}", feat);
            let _ : redis::RedisResult<()> = conn.hset_nx(&chat_key, field, "1");
        }
        match bot.get_chat_administrators(chat_id).await {
            Ok(admins) => {
                for admin in admins {
                    log::info!("Admin: {:?}", admin.user.username);
                    let admin_key = format!("{}:bot_chats", admin.user.id);
                    if !admin.user.is_bot {
                        let _: () = conn
                                .sadd(admin_key, update.chat.id.0)
                                .expect("Failed to add admin to bot_chats");
                        let _: () = conn
                            .sadd(admins_key.clone(), admin.user.id.0)
                            .expect("Failed to add chat to bot_chats");
                    }
                }
            }
            Err(err) => {
                log::error!("Could not fetch admins for {}: {:?}", chat_id, err);
                let _ = bot.send_message(
                    chat_id,
                    "Bot does not have permission to list administrators. \
                    Please make sure I’m still in the group and have the right privileges."
                ).await;
            }
        }
    }
    Ok(())
}

pub async fn run_dispatcher(bot: Bot) {
    // Ensure all default features exist in the global enabled set
    if let Ok(client) = redis::Client::open("redis://127.0.0.1/") {
        if let Ok(mut conn) = client.get_connection() {
            for feat in DEFAULT_FEATURES {
                let _ : redis::RedisResult<()> = conn.sadd(ENABLED_FEATURES_KEY, *feat);
            }
        }
    }

    let commands:Vec<BotCommand> = AdminCommand::bot_commands();
    let _ = bot.set_my_commands(commands).scope(BotCommandScope::Default).await;
    let handler = dptree::entry()
        .branch(Update::filter_message().endpoint(message_handler))
        .branch(
            Update::filter_callback_query()
                .filter(|q: CallbackQuery| {
                    q.data.as_deref().map(|s| s.starts_with("makeadmin:")).unwrap_or(false)
                })
                .endpoint(makeadmin_handler),
        )
        .branch(
            Update::filter_callback_query()
                .filter(|q: CallbackQuery| {
                    q.data.as_deref().map(|s| s.starts_with("stats:")).unwrap_or(false)
                })
                .endpoint(stats_handler),
        )
        .branch(
            // When admin chooses a chat to manage features:
            Update::filter_callback_query()
                .filter(|q: CallbackQuery| {
                    q.data
                        .as_deref()
                        .map(|s| s.starts_with("managefeat:"))
                        .unwrap_or(false)
                })
                .endpoint(manage_features_select_chat),
        )
        .branch(
            // When admin toggles a single feature on/off:
            Update::filter_callback_query()
                .filter(|q: CallbackQuery| {
                    q.data
                        .as_deref()
                        .map(|s| s.starts_with("togglefeat:"))
                        .unwrap_or(false)
                })
                .endpoint(toggle_feature_handler),
        )
        .branch(
            // When admin discards:
            Update::filter_callback_query()
                .filter(|q: CallbackQuery| {
                    q.data
                        .as_deref()
                        .map(|s| s.starts_with("discard:"))
                        .unwrap_or(false)
                })
                .endpoint(discard_handler),
        )
        .branch(Update::filter_chat_member().endpoint(chat_member_handler))
        .branch(Update::filter_my_chat_member().endpoint(my_chat_member_handler));
    Dispatcher::builder(bot, handler).build().dispatch().await;
}