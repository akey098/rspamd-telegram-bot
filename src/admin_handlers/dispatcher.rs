use redis::Commands;
use teloxide::{respond, Bot, RequestError};
use teloxide::dispatching::{Dispatcher, UpdateFilterExt};
use teloxide::dptree;
use teloxide::payloads::AnswerCallbackQuerySetters;
use teloxide::prelude::{CallbackQuery, Message, Requester, ResponseResult, Update};
use teloxide::utils::command::BotCommands;
use crate::admin_handlers::{handle_admin_command, AdminCommand};
use crate::handlers::handle_message;

/// This function will handle incoming messages and dispatch either admin commands or "normal" messages.
pub async fn message_handler(bot: Bot, msg: Message) -> Result<(), RequestError> {
    if let Some(text) = msg.text() {
        // Try to parse an admin command from the message.
        if let Ok(cmd) = AdminCommand::parse(text, "YourBotName") {
            // If an admin command is parsed successfully, handle it.
            handle_admin_command(bot.clone(), msg.clone(), cmd).await?;
        } else {
            // Otherwise, handle this as a regular (possibly spammy) message.
            handle_message(bot.clone(), msg.clone()).await;
        }
    }
    Ok(())
}

/// This function handles callback queries (e.g. inline keyboard actions).
pub async fn callback_handler(bot: Bot, query: CallbackQuery) -> Result<(), RequestError> {
    if let Some(callback_data) = query.data {
        if let Some(admin_chat) = query.message {
            let admin_id = admin_chat.id();  // Use the chat ID of the message
            // Parse the callback data to get the selected chat id:
            let selected_chat: i64 = callback_data.parse().unwrap_or(0);
            if selected_chat != 0 {
                let redis_client = redis::Client::open("redis://127.0.0.1/").expect("Failed to connect to Redis");
                let mut redis_conn = redis_client.get_connection().expect("Failed to get Redis connection");

                // Create a Redis key for this admin's moderated chats:
                let key = format!("admin:{}:moderated_chats", admin_id);
                let _: () = redis_conn.sadd(key, selected_chat)
                    .expect("Failed to add moderated chat to admin");

                bot.answer_callback_query(query.id)
                    .text("Chat assigned for moderation!")
                    .await?;
            }
        }
    }
    Ok(())
}

/// Combines both message and callback handlers into one dispatcher.
pub async fn run_dispatcher(bot: Bot) {
    // Create a combined dptree handler with two branches:
    let handler = dptree::entry()
        .branch(Update::filter_message().endpoint(message_handler))
        .branch(Update::filter_callback_query().endpoint(callback_handler));

    // Build and run the dispatcher. Note that we no longer use the `.callbacks(...)` method.
    Dispatcher::builder(bot, handler)
        .build()
        .dispatch()
        .await;
}
