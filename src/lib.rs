pub mod admin_handlers;
pub mod admin_panel;
pub mod handlers;
pub mod config;
pub mod ban_manager;
pub mod migration;
pub mod trust_manager;
pub mod fuzzy_trainer;
pub mod bayes_manager;

use anyhow::Result;
use redis::Connection;

/// Get a Redis connection
pub async fn get_redis_connection() -> Result<Connection> {
    let redis_client = redis::Client::open("redis://127.0.0.1/")?;
    let redis_conn = redis_client.get_connection()?;
    Ok(redis_conn)
}