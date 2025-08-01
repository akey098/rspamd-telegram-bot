use redis::Commands;
use std::error::Error;
use crate::config::{field, key};

/// Migrate reputation data from the old system to the new Rspamd reputation system
pub async fn migrate_reputation_data() -> Result<(), Box<dyn Error + Send + Sync>> {
    let redis_client = redis::Client::open("redis://127.0.0.1/")?;
    let mut redis_conn = redis_client.get_connection()?;
    
    println!("Starting reputation data migration...");
    
    // Get all user keys
    let user_keys: Vec<String> = redis_conn.keys(format!("{}*", key::TG_USERS_PREFIX))?;
    println!("Found {} user keys to migrate", user_keys.len());
    
    let mut migrated_count = 0;
    let mut skipped_count = 0;
    
    for user_key in user_keys {
        // Extract user ID from key (remove the prefix)
        let user_id = user_key.replace(&format!("{}", key::TG_USERS_PREFIX), "");
        
        // Get existing reputation
        let rep: Option<i64> = redis_conn.hget(&user_key, field::REP)?;
        
        if let Some(rep_value) = rep {
            if rep_value != 0 {
                // Convert to Rspamd reputation format
                let reputation_key = format!("tg:reputation:user:{}", user_id);
                
                if rep_value > 0 {
                    // Positive reputation becomes bad reputation (spam behavior)
                    let _: () = redis_conn.hset(&reputation_key, "bad", rep_value)?;
                    let _: () = redis_conn.hset(&reputation_key, "good", 0)?;
                    println!("Migrated user {}: {} bad reputation points", user_id, rep_value);
                } else {
                    // Negative reputation becomes good reputation (legitimate behavior)
                    let _: () = redis_conn.hset(&reputation_key, "good", rep_value.abs())?;
                    let _: () = redis_conn.hset(&reputation_key, "bad", 0)?;
                    println!("Migrated user {}: {} good reputation points", user_id, rep_value.abs());
                }
                
                // Set expiration for the reputation key (1 week)
                let _: () = redis_conn.expire(&reputation_key, 604800)?;
                
                migrated_count += 1;
            } else {
                skipped_count += 1;
            }
        } else {
            skipped_count += 1;
        }
    }
    
    println!("Migration completed: {} users migrated, {} users skipped", migrated_count, skipped_count);
    
    Ok(())
}

/// Verify that the migration was successful by checking a sample of users
pub async fn verify_migration() -> Result<(), Box<dyn Error + Send + Sync>> {
    let redis_client = redis::Client::open("redis://127.0.0.1/")?;
    let mut redis_conn = redis_client.get_connection()?;
    
    println!("Verifying migration...");
    
    // Get a sample of user keys
    let user_keys: Vec<String> = redis_conn.keys(format!("{}*", key::TG_USERS_PREFIX))?;
    let sample_size = std::cmp::min(10, user_keys.len());
    let sample_keys = &user_keys[..sample_size];
    
    for user_key in sample_keys {
        let user_id = user_key.replace(&format!("{}", key::TG_USERS_PREFIX), "");
        let reputation_key = format!("tg:reputation:user:{}", user_id);
        
        // Check if reputation key exists
        let exists: bool = redis_conn.exists(&reputation_key)?;
        
        if exists {
            let bad: i64 = redis_conn.hget(&reputation_key, "bad").unwrap_or(0);
            let good: i64 = redis_conn.hget(&reputation_key, "good").unwrap_or(0);
            println!("User {}: bad={}, good={}", user_id, bad, good);
        } else {
            println!("User {}: no reputation data found", user_id);
        }
    }
    
    Ok(())
}

/// Clean up old reputation data after successful migration
pub async fn cleanup_old_reputation_data() -> Result<(), Box<dyn Error + Send + Sync>> {
    let redis_client = redis::Client::open("redis://127.0.0.1/")?;
    let mut redis_conn = redis_client.get_connection()?;
    
    println!("Cleaning up old reputation data...");
    
    // Get all user keys
    let user_keys: Vec<String> = redis_conn.keys(format!("{}*", key::TG_USERS_PREFIX))?;
    
    let mut cleaned_count = 0;
    
    for user_key in user_keys {
        // Remove the old 'rep' field from user hashes
        let removed: i64 = redis_conn.hdel(&user_key, field::REP)?;
        if removed > 0 {
            cleaned_count += 1;
        }
    }
    
    println!("Cleanup completed: {} users cleaned", cleaned_count);
    
    Ok(())
} 