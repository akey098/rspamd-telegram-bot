use std::env;
use teloxide::prelude::*;
use dotenv::*;

mod handlers;
mod admin;
mod commands;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    dotenv().ok();
    log::info!("Starting the spam detection bot...");

    let bot_token = env::var("BOT_TOKEN").expect("BOT_TOKEN must be set in .env file");

    let bot = Bot::new(bot_token);

    use redis::Commands;
    let client = redis::Client::open("redis://127.0.0.1/")?;
    let mut conn = client.get_connection()?;

    run_dispatcher(bot).await;

    teloxide::repl(bot, move |bot: Bot, message: Message| {
        async move {
            if let Err(e) = handlers::handle_message(bot.clone(), message).await {
                eprintln!("Error handling message: {:?}", e);
            }
            respond(())
        }
    })
        .await;

    
}
