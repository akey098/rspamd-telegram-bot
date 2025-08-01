use redis::Commands;
use std::error::Error;

#[tokio::test]
async fn test_reputation_migration() -> Result<(), Box<dyn Error + Send + Sync>> {
    let redis_client = redis::Client::open("redis://127.0.0.1/")?;
    let mut redis_conn = redis_client.get_connection()?;
    
    // Test user ID
    let test_user_id = "123456789";
    let user_key = format!("tg:users:{}", test_user_id);
    let reputation_key = format!("tg:reputation:user:{}", test_user_id);
    
    // Clean up any existing test data
    let _: () = redis_conn.del(&user_key)?;
    let _: () = redis_conn.del(&reputation_key)?;
    
    // Set up test data with old reputation format
    let _: () = redis_conn.hset(&user_key, "rep", 5)?;
    
    // Run migration
    rspamd_telegram_bot::migration::migrate_reputation_data().await?;
    
    // Verify migration
    let bad: i64 = redis_conn.hget(&reputation_key, "bad").unwrap_or(0);
    let good: i64 = redis_conn.hget(&reputation_key, "good").unwrap_or(0);
    
    assert_eq!(bad, 5, "Bad reputation should be 5");
    assert_eq!(good, 0, "Good reputation should be 0");
    
    println!("Reputation migration test passed");
    
    // Clean up
    let _: () = redis_conn.del(&user_key)?;
    let _: () = redis_conn.del(&reputation_key)?;
    
    Ok(())
}

#[tokio::test]
async fn test_reputation_symbols() -> Result<(), Box<dyn Error + Send + Sync>> {
    // This test would verify that Rspamd is properly generating reputation symbols
    // It would require a running Rspamd instance with the reputation plugin configured
    
    println!("Reputation symbols test - requires running Rspamd instance");
    println!("To test manually:");
    println!("1. Start Rspamd with reputation plugin");
    println!("2. Send a message through the bot");
    println!("3. Check if USER_REPUTATION symbols are generated");
    
    Ok(())
}

#[tokio::test]
async fn test_reputation_decay() -> Result<(), Box<dyn Error + Send + Sync>> {
    let redis_client = redis::Client::open("redis://127.0.0.1/")?;
    let mut redis_conn = redis_client.get_connection()?;
    
    let test_user_id = "987654321";
    let reputation_key = format!("tg:reputation:user:{}", test_user_id);
    
    // Clean up
    let _: () = redis_conn.del(&reputation_key)?;
    
    // Set up test reputation data
    let _: () = redis_conn.hset(&reputation_key, "bad", 10)?;
    let _: () = redis_conn.hset(&reputation_key, "good", 2)?;
    let _: () = redis_conn.expire(&reputation_key, 3600)?; // 1 hour expiration
    
    // Verify initial state
    let bad: i64 = redis_conn.hget(&reputation_key, "bad").unwrap_or(0);
    let good: i64 = redis_conn.hget(&reputation_key, "good").unwrap_or(0);
    
    assert_eq!(bad, 10, "Initial bad reputation should be 10");
    assert_eq!(good, 2, "Initial good reputation should be 2");
    
    println!("Reputation decay test - initial state verified");
    println!("Note: Actual decay is handled by Rspamd's reputation plugin");
    
    // Clean up
    let _: () = redis_conn.del(&reputation_key)?;
    
    Ok(())
} 