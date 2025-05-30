use std::collections::HashMap;
use crate::admin_handlers::{handle_admin_command, AdminCommand};
use crate::handlers::handle_message;
use redis::{Commands, RedisResult};
use teloxide::dispatching::{Dispatcher, UpdateFilterExt};
use teloxide::dptree;
use teloxide::payloads::{AnswerCallbackQuerySetters, RestrictChatMemberSetters};
use teloxide::prelude::{CallbackQuery, ChatId, ChatMemberUpdated, Message, Requester, Update, UserId};
use teloxide::types::{BotCommand, ChatKind, ChatMemberStatus, ChatPermissions};
use teloxide::utils::command::BotCommands;
use teloxide::{Bot, RequestError};
use std::fmt::Write;
use chrono::{Duration, Utc};
use teloxide::sugar::bot::BotMessagesExt;
use crate::config::{field, key, suffix};

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
        
        if let Ok(cmd) = AdminCommand::parse(text, "rspamd-bot") {
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
            let selected_chat: i64 = callback_data["makeadmin:".len()..].parse().unwrap();
            let redis_client =
                redis::Client::open("redis://127.0.0.1/").expect("Failed to connect to Redis");
            let mut redis_conn = redis_client
                .get_connection()
                .expect("Failed to get Redis connection");
            if selected_chat != 0 {
                let stats: HashMap<String, String> = redis_conn
                    .hgetall(format!("{}{}", key::TG_CHATS_PREFIX, selected_chat))
                    .expect("Failed to get chat stats");
                let mut response = String::new();
                for (field, value) in stats {
                    if field == field::NAME || field == field::ADMIN_CHAT {
                        continue;
                    }
                    writeln!(&mut response, "{}: {}", field, value).unwrap();
                }
                bot.send_message(admin_id, response).await?;
            } else {
                let chats: Vec<i64> = redis_conn
                    .smembers(format!("{}{}{}", key::ADMIN_PREFIX, admin_id, suffix::MODERATED_CHATS))
                    .expect("Failed to get moderated chats");
                let mut total_stats: HashMap<String, i64> = HashMap::new();
                for chat in chats {
                    let chat_stats: HashMap<String, String> = redis_conn
                        .hgetall(format!("{}{}", key::TG_CHATS_PREFIX, chat))
                        .expect("Failed to get chat stats");
                
                    for (key, value_str) in chat_stats {
                        if key == field::NAME || key == field::ADMIN_CHAT {
                            continue;
                        }
                        if let Ok(value) = value_str.parse::<i64>() {
                            *total_stats.entry(key).or_insert(0) += value;
                        }
                    }
                }
                let mut resp = String::new();
                for (k, v) in total_stats {
                    writeln!(&mut resp, "{}: {}", k, v).unwrap();
                }
                bot.send_message(admin_id, resp).await?;
            }
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
                        .expect("Failed to add chat to bot_chats");
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
            .hset(chat_key, field::NAME, update.chat.title().unwrap())
            .expect("Failed to set up chat key");
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
    let commands:Vec<BotCommand> = AdminCommand::bot_commands();
    let _ = bot.set_my_commands(commands).await;
    let handler = dptree::entry()
        .branch(Update::filter_message().endpoint(message_handler))
        .branch(
            Update::filter_callback_query()
                .filter(|q: &CallbackQuery| {
                    q.data.as_deref().map(|s| s.starts_with("makeadmin:")).unwrap_or(false)
                })
                .endpoint(makeadmin_handler),
        )
        .branch(
            Update::filter_callback_query()
                .filter(|q: &CallbackQuery| {
                    q.data.as_deref().map(|s| s.starts_with("stats:")).unwrap_or(false)
                })
                .endpoint(stats_handler),
        )
        .branch(Update::filter_chat_member().endpoint(chat_member_handler))
        .branch(Update::filter_my_chat_member().endpoint(my_chat_member_handler));
    Dispatcher::builder(bot, handler).build().dispatch().await;
}