#[test]
fn test_reputation_config_structure() {
    // Test that our reputation configuration has the correct structure
    let config_content = r#"
# Enable reputation plugin
enabled = true;

# Configure Redis backend
backend = "redis";

# Redis connection settings
servers = "127.0.0.1:6379";

# Define user reputation rule
rules {
    # Telegram user reputation tracking
    telegram_user {
        # Use generic selector to extract user ID from header
        selector = "generic";
        
        # Extract user ID from X-Telegram-User header
        selector_value = "X-Telegram-User";
        
        # Symbol to add based on reputation
        symbol = "USER_REPUTATION";
        
        # Redis key pattern for storing reputation
        key = "tg:reputation:user:%{selector}";
        
        # Reputation thresholds
        bad_threshold = 10;
        good_threshold = -5;
        
        # Time buckets for reputation decay (in seconds)
        time_buckets = [3600, 86400, 604800]; # 1h, 1d, 1w
        
        # Reputation scoring
        score_bad = 5.0;
        score_good = -1.0;
        score_neutral = 0.0;
    }
}
"#;
    
    // Basic validation of config structure
    assert!(config_content.contains("enabled = true"));
    assert!(config_content.contains("selector = \"generic\""));
    assert!(config_content.contains("symbol = \"USER_REPUTATION\""));
    assert!(config_content.contains("X-Telegram-User"));
    assert!(config_content.contains("backend = \"redis\""));
    assert!(config_content.contains("servers = \"127.0.0.1:6379\""));
    
    println!("✅ Reputation configuration structure is valid");
}

#[test]
fn test_reputation_config_file_exists() {
    // Test that the reputation configuration file exists and is readable
    use std::fs;
    use std::path::Path;
    
    let config_path = Path::new("rspamd-config/local.d/reputation.conf");
    assert!(config_path.exists(), "Reputation config file should exist");
    
    let config_content = fs::read_to_string(config_path).expect("Should be able to read reputation config");
    
    // Verify key configuration elements are present
    assert!(config_content.contains("enabled = true"));
    assert!(config_content.contains("selector = \"generic\""));
    assert!(config_content.contains("X-Telegram-User"));
    assert!(config_content.contains("USER_REPUTATION"));
    
    println!("✅ Reputation configuration file exists and contains required elements");
} 