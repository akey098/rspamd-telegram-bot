pub async fn handle_callback_query(bot: Bot, query: CallbackQuery) -> ResponseResult<()> {
    if let Some(callback_data) = query.data {
        if let Some(admin_chat) = query.message {
            let admin_id = admin_chat.chat.id.0;
            let selected_chat: i64 = callback_data.parse().unwrap_or(0);
            if selected_chat != 0 {
                let redis_client = redis::Client::open("redis://127.0.0.1/").expect("Failed to connect to Redis");
                let mut redis_conn = redis_client.get_connection().expect("Failed to get Redis connection");

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


async fn run_dispatcher(bot: Bot) {
    Dispatcher::new(bot.clone())
        .messages_handler(|rx| {
            rx.for_each_concurrent(None, move |msg| {
                let bot = bot.clone();
                async move {
                    if let Some(text) = msg.update.text() {
                        // Parse admin commands and dispatch if needed.
                        // Replace "YourBotName" with your bot's username.
                        match AdminCommand::parse(text, "YourBotName") {
                            Ok(cmd) => {
                                if let Err(err) = handle_admin_command(bot.clone(), msg.update.clone(), cmd).await {
                                    log::error!("Error handling admin command: {:?}", err);
                                }
                            }
                            Err(_e) => {
                                // Optionally, this may be a normal (non-admin) message.
                                // Here you could call your spam-handling function.
                                handle_spammy_message(&bot, &msg.update).await.ok();
                            }
                        }
                    }
                }
            })
        })
        .callback_queries_handler(|rx| {
            rx.for_each_concurrent(None, move |query| {
                let bot = bot.clone();
                async move {
                    if let Err(err) = handle_callback_query(bot.clone(), query.update).await {
                        log::error!("Error handling callback query: {:?}", err);
                    }
                }
            })
        })
        .dispatch()
        .await;
}