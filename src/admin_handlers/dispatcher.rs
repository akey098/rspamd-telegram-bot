use crate::admin_handlers::{handle_admin_command, AdminCommand};
use crate::handlers::handle_message;
use redis::{Commands, RedisResult};
use teloxide::dispatching::{Dispatcher, UpdateFilterExt};
use teloxide::dptree;
use teloxide::payloads::AnswerCallbackQuerySetters;
use teloxide::prelude::{CallbackQuery, ChatId, ChatMemberUpdated, Message, Requester, Update};
use teloxide::types::ChatMemberStatus;
use teloxide::utils::command::BotCommands;
use teloxide::{Bot, RequestError};

pub async fn message_handler(bot: Bot, msg: Message) -> Result<(), RequestError> {
    if let Some(text) = msg.text() {
        let client = redis::Client::open("redis://127.0.0.1/").expect("failed to get redis client.");
        let mut conn = client.get_connection().expect("Failed to connect");
        let msg_cloned = msg.clone();
        let username = msg_cloned.from.unwrap().username.unwrap();
        let key = format!("tg:{}:rep", username);

        let user_rep: RedisResult<i64> = conn.get(&key);

        match user_rep {
            Ok(_) => {}
            Err(_) => {
                let _: () = conn
                    .set(key, 0)
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

pub async fn callback_handler(bot: Bot, query: CallbackQuery) -> Result<(), RequestError> {
    if let Some(callback_data) = query.data {
        if let Some(admin_chat) = query.message {
            let admin_id = admin_chat.chat().id;
            let selected_chat: i64 = callback_data.parse().unwrap_or(0);
            if selected_chat != 0 {
                let redis_client =
                    redis::Client::open("redis://127.0.0.1/").expect("Failed to connect to Redis");
                let mut redis_conn = redis_client
                    .get_connection()
                    .expect("Failed to get Redis connection");

                let key = format!("admin:{}:moderated_chats", admin_id);
                let _: () = redis_conn
                    .sadd(key, selected_chat)
                    .expect("Failed to add moderated chat to admin");
                let _: () = redis_conn
                    .sadd(format!("chat:{}:admin_chat", selected_chat), admin_id.0)
                    .expect("Failed to add admin chat to selected chat");

                bot.answer_callback_query(query.id)
                    .text("Chat assigned for moderation!")
                    .await?;
            }
        }
    }
    Ok(())
}

pub async fn run_dispatcher(bot: Bot) {
    let handler = dptree::entry()
        .branch(Update::filter_message().endpoint(message_handler))
        .branch(Update::filter_callback_query().endpoint(callback_handler))
        .branch(Update::filter_my_chat_member().endpoint(my_chat_member_handler));

    Dispatcher::builder(bot, handler).build().dispatch().await;
}


pub async fn my_chat_member_handler(
    bot: Bot,
    update: ChatMemberUpdated,
) -> Result<(), RequestError> {
    let new_status = update.new_chat_member.status();
    let chat_id = ChatId(update.chat.id.0);

    // Open Redis once
    let client = redis::Client::open("redis://127.0.0.1/").expect("failed to get redis client.");
    let mut conn = client.get_connection().expect("Failed to connect");
    let key = format!("tg:{}:rep", update.new_chat_member.user.username.unwrap().to_string());
    let admin_key = format!("{}:bot_chats", update.new_chat_member.user.id);

    match new_status {
        ChatMemberStatus::Member | ChatMemberStatus::Administrator | ChatMemberStatus::Owner => {
            if new_status == ChatMemberStatus::Administrator || new_status == ChatMemberStatus::Owner {
                let _: () = conn
                    .sadd(admin_key, chat_id.0)
                    .expect("Failed to add chat to bot_chats");
            }
            let _: () = conn
                .set(key, 0)
                .expect("Failed to update user's reputation");
        }
        ChatMemberStatus::Left | ChatMemberStatus::Banned => {
            if update.old_chat_member.status() == ChatMemberStatus::Administrator || update.old_chat_member.status() == ChatMemberStatus::Owner {
                let _: () = conn
                    .srem(admin_key, chat_id.0)
                    .expect("Failed to remove chat from bot_chats");
            }
            let _: () = conn
                .del(key)
                .expect("Failed to remove user's reputation");
        }
        _ => {}
    }

    Ok(())
}
