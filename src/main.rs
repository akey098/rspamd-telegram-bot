use dotenv::*;
use std::env;
use teloxide::prelude::*;

mod admin_handlers;
mod handlers;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    dotenv().ok();
    log::info!("Starting the spam detection bot...");

    let bot_token = env::var("BOT_TOKEN").expect("BOT_TOKEN must be set in .env file");

    let bot = Bot::new(bot_token);

    admin_handlers::run_dispatcher(bot).await;
}
