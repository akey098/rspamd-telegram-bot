use crate::config::{field, key, BAN_COUNTER_REDUCTION_INTERVAL};
use redis::Commands;
use std::error::Error;
use tokio::time::{sleep, Duration};
use chrono::Utc;

pub struct BanManager {
    redis_client: redis::Client,
}

impl BanManager {
    pub fn new() -> Result<Self, Box<dyn Error + Send + Sync>> {
        let redis_client = redis::Client::open("redis://127.0.0.1/")?;
        Ok(BanManager { redis_client })
    }

    pub async fn start_ban_counter_reduction(&self) {
        loop {
            if let Err(e) = self.reduce_ban_counters().await {
                eprintln!("Error reducing ban counters: {}", e);
            }
            
            // Sleep for 1 hour before next check
            sleep(Duration::from_secs(3600)).await;
        }
    }

    async fn reduce_ban_counters(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut redis_conn = self.redis_client.get_connection()?;
        let current_time = Utc::now().timestamp();
        
        // Get all user keys
        let user_keys: Vec<String> = redis_conn.keys(&format!("{}*", key::TG_USERS_PREFIX))?;
        
        for user_key in user_keys {
            // Check if this user has a ban reduction time set
            let reduction_time_str: Option<String> = redis_conn.hget(&user_key, "ban_reduction_time")?;
            
            if let Some(reduction_time_str) = reduction_time_str {
                if let Ok(reduction_time) = reduction_time_str.parse::<i64>() {
                    // If it's time to reduce the ban counter
                    if current_time >= reduction_time {
                        let banned_q: i64 = redis_conn.hget(&user_key, field::BANNED_Q)?;
                        
                        if banned_q > 0 {
                            // Reduce ban counter by 1
                            let new_banned_q = banned_q - 1;
                            let _: () = redis_conn.hset(&user_key, field::BANNED_Q, new_banned_q)?;
                            
                            // Set next reduction time
                            let next_reduction_time = current_time + BAN_COUNTER_REDUCTION_INTERVAL;
                            let _: () = redis_conn.hset(&user_key, "ban_reduction_time", next_reduction_time.to_string())?;
                            
                            println!("Reduced ban counter for user {} from {} to {}", 
                                   user_key, banned_q, new_banned_q);
                        } else {
                            // Remove the reduction time field if ban counter is already 0
                            let _: () = redis_conn.hdel(&user_key, "ban_reduction_time")?;
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
} 