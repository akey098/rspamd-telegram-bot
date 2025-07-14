use std::time::Duration;
use std::error::Error; 
use redis::Commands;
use teloxide::prelude::*;
use tokio::time;
use rspamd_telegram_bot::config::{field, key};
use rspamd_telegram_bot::admin_handlers;

#[tokio::main]
async fn main() {
    println!("=== REAL TELEGRAM BOT STARTING ===");
    
    // Load environment variables from .env file
    dotenv::dotenv().ok();
    
    pretty_env_logger::init();
    log::info!("Starting the spam detection bot...");

    let bot = Bot::from_env();

    tokio::spawn({
        async move {
            let mut interval = time::interval(Duration::from_secs(3600));
            loop {
                interval.tick().await;
                if let Err(err) = do_periodic().await {
                    log::error!("Periodic task failed: {:?}", err);
                }
            }
        }
    });

    admin_handlers::run_dispatcher(bot).await;
}

async fn do_periodic() -> Result<(), Box<dyn Error + Send + Sync>> {
    let redis_client = redis::Client::open("redis://127.0.0.1/")
        .expect("Failed to connect to Redis");
    let mut redis_conn = redis_client
        .get_connection()
        .expect("Failed to get Redis connection");

    let keys: Vec<String> = redis_conn
        .keys(format!("{}*", key::TG_USERS_PREFIX))
        .expect("Failed to get users keys");

    for key in keys {
        let rep: i64 = redis_conn
            .hget(key.clone(), field::REP)
            .expect("Failed to get user's reputation");

        if rep > 0 {
            let _: () = redis_conn
                .hincr(key.clone(), field::REP, -1)
                .expect("Failed to decrease user's reputation");
        }
    }

    Ok(())
}