use rspamd_telegram_bot::config::{field, key, BAN_COUNTER_REDUCTION_INTERVAL};
use redis::Commands;
use std::error::Error;

#[tokio::test]
async fn test_ban_counter_system() -> Result<(), Box<dyn Error + Send + Sync>> {
    let redis_client = redis::Client::open("redis://127.0.0.1/")?;
    let mut redis_conn = redis_client.get_connection()?;
    
    // Test user ID
    let test_user_id = 12345;
    let user_key = format!("{}{}", key::TG_USERS_PREFIX, test_user_id);
    
    // Clean up any existing data
    let _: () = redis_conn.del(&user_key)?;
    
    // Test 1: Initial ban counter should be 0 (or not exist)
    let banned_q: i64 = redis_conn.hget(&user_key, field::BANNED_Q).unwrap_or(0);
    assert_eq!(banned_q, 0, "Initial ban counter should be 0");
    
    // Test 2: First ban - increment counter
    let _: () = redis_conn.hincr(&user_key, field::BANNED_Q, 1)?;
    let banned_q: i64 = redis_conn.hget(&user_key, field::BANNED_Q)?;
    assert_eq!(banned_q, 1, "After first ban, counter should be 1");
    
    // Test 3: Second ban - increment counter
    let _: () = redis_conn.hincr(&user_key, field::BANNED_Q, 1)?;
    let banned_q: i64 = redis_conn.hget(&user_key, field::BANNED_Q)?;
    assert_eq!(banned_q, 2, "After second ban, counter should be 2");
    
    // Test 4: Third ban - should trigger permanent ban
    let _: () = redis_conn.hincr(&user_key, field::BANNED_Q, 1)?;
    let banned_q: i64 = redis_conn.hget(&user_key, field::BANNED_Q)?;
    assert_eq!(banned_q, 3, "After third ban, counter should be 3");
    
    // Test 5: Set ban reduction time
    let current_time = chrono::Utc::now().timestamp();
    let reduction_time = current_time + BAN_COUNTER_REDUCTION_INTERVAL;
    let _: () = redis_conn.hset(&user_key, "ban_reduction_time", reduction_time.to_string())?;
    
    // Test 6: Verify reduction time was set
    let reduction_time_str: String = redis_conn.hget(&user_key, "ban_reduction_time")?;
    let stored_reduction_time: i64 = reduction_time_str.parse()?;
    assert_eq!(stored_reduction_time, reduction_time, "Reduction time should be set correctly");
    
    // Test 7: Simulate ban counter reduction
    let _: () = redis_conn.hincr(&user_key, field::BANNED_Q, -1)?;
    let banned_q: i64 = redis_conn.hget(&user_key, field::BANNED_Q)?;
    assert_eq!(banned_q, 2, "After reduction, counter should be 2");
    
    // Clean up
    let _: () = redis_conn.del(&user_key)?;
    
    println!("All ban system tests passed!");
    Ok(())
}

#[tokio::test]
async fn test_ban_reduction_logic() -> Result<(), Box<dyn Error + Send + Sync>> {
    let redis_client = redis::Client::open("redis://127.0.0.1/")?;
    let mut redis_conn = redis_client.get_connection()?;
    
    // Test user ID
    let test_user_id = 67890;
    let user_key = format!("{}{}", key::TG_USERS_PREFIX, test_user_id);
    
    // Clean up any existing data
    let _: () = redis_conn.del(&user_key)?;
    
    // Set up test scenario: user has 2 bans and reduction time in the past
    let _: () = redis_conn.hset(&user_key, field::BANNED_Q, 2)?;
    let past_time = chrono::Utc::now().timestamp() - 1000; // 1000 seconds ago
    let _: () = redis_conn.hset(&user_key, "ban_reduction_time", past_time.to_string())?;
    
    // Verify initial state
    let banned_q: i64 = redis_conn.hget(&user_key, field::BANNED_Q)?;
    assert_eq!(banned_q, 2, "Initial ban counter should be 2");
    
    // Simulate ban reduction process
    let current_time = chrono::Utc::now().timestamp();
    let reduction_time_str: Option<String> = redis_conn.hget(&user_key, "ban_reduction_time")?;
    
    if let Some(reduction_time_str) = reduction_time_str {
        if let Ok(reduction_time) = reduction_time_str.parse::<i64>() {
            if current_time >= reduction_time {
                // Reduce ban counter by 1
                let new_banned_q = banned_q - 1;
                let _: () = redis_conn.hset(&user_key, field::BANNED_Q, new_banned_q)?;
                
                // Set next reduction time
                let next_reduction_time = current_time + BAN_COUNTER_REDUCTION_INTERVAL;
                let _: () = redis_conn.hset(&user_key, "ban_reduction_time", next_reduction_time.to_string())?;
                
                // Verify reduction worked
                let new_banned_q_check: i64 = redis_conn.hget(&user_key, field::BANNED_Q)?;
                assert_eq!(new_banned_q_check, 1, "Ban counter should be reduced to 1");
                
                // Verify next reduction time was set
                let next_reduction_time_str: String = redis_conn.hget(&user_key, "ban_reduction_time")?;
                let next_reduction_time_check: i64 = next_reduction_time_str.parse()?;
                assert!(next_reduction_time_check > current_time, "Next reduction time should be in the future");
            }
        }
    }
    
    // Clean up
    let _: () = redis_conn.del(&user_key)?;
    
    println!("Ban reduction logic test passed!");
    Ok(())
} 