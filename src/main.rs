use std::time::Duration;
use std::error::Error; 
use redis::Commands;
use teloxide::prelude::*;
use tokio::time;
use rspamd_telegram_bot::config::{field, key};
use rspamd_telegram_bot::admin_handlers;
use std::env;

#[tokio::main]
async fn main() {
    println!("=== REAL TELEGRAM BOT STARTING ===");
    
    // Load environment variables from .env file
    dotenv::dotenv().ok();
    
    pretty_env_logger::init();
    log::info!("Starting the spam detection bot...");

    let bot = Bot::from_env();

    // Start health check server for Render (if PORT is set)
    if let Ok(port) = env::var("PORT") {
        tokio::spawn(start_health_server(port));
    }

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

async fn start_health_server(port: String) {
    use warp::Filter;
    
    let health = warp::path("health")
        .map(|| warp::reply::with_status("OK", warp::http::StatusCode::OK));
    
    let root = warp::path::end()
        .map(|| warp::reply::with_status("Telegram Bot Running", warp::http::StatusCode::OK));
    
    let routes = health.or(root);
    
    let port: u16 = port.parse().unwrap_or(3000);
    println!("Health server starting on 0.0.0.0:{}", port);
    
    warp::serve(routes)
        .run(([0, 0, 0, 0], port))
        .await;
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
        let rep: Option<i64> = redis_conn
            .hget(key.clone(), field::REP)
            .expect("Failed to get user's reputation");

        if let Some(rep_value) = rep {
            if rep_value > 0 {
                let _: () = redis_conn
                    .hincr(key.clone(), field::REP, -1)
                    .expect("Failed to decrease user's reputation");
            }
        } else {
            // User doesn't have a reputation field yet, initialize it to 0
            let _: () = redis_conn
                .hset(key.clone(), field::REP, 0)
                .expect("Failed to initialize user's reputation");
        }
    }

    Ok(())
}