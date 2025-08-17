# BayesManager Documentation

The `BayesManager` module provides Bayesian learning capabilities for the Rspamd Telegram bot. It manages the interaction between the bot and Rspamd's Bayesian classifier for adaptive spam detection.

## Overview

The BayesManager handles:
- Learning messages as spam or ham via Rspamd HTTP API
- Tracking learning statistics in Redis
- Monitoring classifier readiness
- Managing learned message tracking

## Features

### Core Functionality

1. **Message Learning**: Learn messages as spam or ham using Rspamd's HTTP API
2. **Statistics Tracking**: Monitor classifier performance and training progress
3. **Readiness Detection**: Determine when the classifier has enough training data
4. **Message Tracking**: Prevent duplicate learning of the same message
5. **Data Management**: Reset classifier data when needed

### Configuration

The BayesManager uses configuration constants defined in `src/config.rs`:

```rust
pub mod bayes {
    pub const BAYES_SPAM_KEY: &str = "bayes_spam";
    pub const BAYES_HAM_KEY: &str = "bayes_ham";
    pub const BAYES_SPAM_MESSAGES_KEY: &str = "bayes_spam_messages";
    pub const BAYES_HAM_MESSAGES_KEY: &str = "bayes_ham_messages";
    pub const BAYES_LEARNED_PREFIX: &str = "bayes:learned:";
    pub const MIN_SPAM_MESSAGES: i64 = 200;
    pub const MIN_HAM_MESSAGES: i64 = 200;
    pub const LEARNED_EXPIRY: u64 = 86400; // 24 hours
}
```

## Usage

### Basic Usage

```rust
use rspamd_telegram_bot::bayes_manager::BayesManager;

// Create a new BayesManager instance
let bayes_manager = BayesManager::new()?;

// Learn a message as spam
bayes_manager.learn_spam("message_id_123", "Buy now! Limited time offer!").await?;

// Learn a message as ham
bayes_manager.learn_ham("message_id_456", "Hello, how are you today?").await?;

// Check if the classifier is ready
let is_ready = bayes_manager.is_ready()?;
if is_ready {
    println!("Classifier is ready for effective classification");
} else {
    println!("Classifier needs more training data");
}
```

### Statistics and Monitoring

```rust
// Get basic statistics
let stats = bayes_manager.get_bayes_stats()?;
println!("Spam tokens: {}", stats.get("spam_tokens").unwrap_or(&0));
println!("Ham tokens: {}", stats.get("ham_tokens").unwrap_or(&0));
println!("Spam messages: {}", stats.get("spam_messages").unwrap_or(&0));
println!("Ham messages: {}", stats.get("ham_messages").unwrap_or(&0));
println!("Total messages: {}", stats.get("total_messages").unwrap_or(&0));
println!("Spam ratio: {}%", stats.get("spam_ratio_percent").unwrap_or(&0));

// Get detailed information
let detailed_info = bayes_manager.get_detailed_info()?;
println!("Status: {}", detailed_info.get("status").unwrap());
println!("Spam progress: {}%", detailed_info.get("spam_progress_percent").unwrap());
println!("Ham progress: {}%", detailed_info.get("ham_progress_percent").unwrap());
```

### Message Tracking

```rust
// Check if a message has been learned
let message_id = "some_message_id";
let is_learned = bayes_manager.is_message_learned(message_id)?;
if is_learned {
    println!("Message has already been learned");
    
    // Get the learning type
    let learning_type = bayes_manager.get_message_learning_type(message_id)?;
    match learning_type {
        Some(learning_type) => println!("Learned as: {}", learning_type),
        None => println!("Message not found in learning records"),
    }
} else {
    println!("Message has not been learned yet");
}
```

### Data Management

```rust
// Reset all Bayesian classifier data (use with caution)
bayes_manager.reset_all_data()?;
println!("All Bayesian data has been reset");
```

## Requirements

### Rspamd Configuration

The BayesManager requires Rspamd to be configured with the Bayes module enabled. Ensure you have:

1. **Bayes module enabled** in Rspamd configuration
2. **Redis backend** configured for token storage
3. **HTTP controller** accessible at the configured URL
4. **Proper authentication** with the configured password

### Redis Storage

The BayesManager stores data in Redis using the following key patterns:

- `bayes_spam`: Set of spam tokens
- `bayes_ham`: Set of ham tokens  
- `bayes_spam_messages`: Counter for spam messages
- `bayes_ham_messages`: Counter for ham messages
- `bayes:learned:spam:<message_id>`: Spam learning records
- `bayes:learned:ham:<message_id>`: Ham learning records

## Error Handling

The BayesManager uses `anyhow::Result` for error handling. Common error scenarios include:

- **Rspamd connection failures**: Network issues or Rspamd service unavailable
- **Redis connection failures**: Redis service unavailable or connection issues
- **Authentication failures**: Incorrect Rspamd password
- **Invalid responses**: Unexpected response format from Rspamd

## Testing

The BayesManager includes comprehensive tests:

```bash
# Run unit tests
cargo test bayes_manager --lib

# Run integration tests
cargo test --test bayes_integration_test
```

## Integration with Bot

The BayesManager is designed to integrate with the existing bot architecture:

1. **Auto-learning**: Messages with high spam scores can be automatically learned as spam
2. **Manual learning**: Admins can manually learn messages using bot commands
3. **Statistics**: Bot can report classifier status and performance
4. **Monitoring**: Track classifier readiness and training progress

## Performance Considerations

- **Redis connections**: The BayesManager creates new Redis connections for each operation
- **HTTP requests**: Learning operations make HTTP requests to Rspamd
- **Message tracking**: Learning records expire after 24 hours to prevent memory bloat
- **Statistics**: Statistics are calculated on-demand from Redis data

## Future Enhancements

Potential improvements for the BayesManager:

1. **Connection pooling**: Reuse Redis connections for better performance
2. **Batch operations**: Support for learning multiple messages at once
3. **Advanced statistics**: More detailed performance metrics
4. **Configuration validation**: Validate Rspamd configuration on startup
5. **Health checks**: Periodic validation of Rspamd connectivity
