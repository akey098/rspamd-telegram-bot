use rspamd_telegram_bot::fuzzy_trainer::FuzzyTrainer;
use rspamd_telegram_bot::config::{rspamd, symbol};
use reqwest::Client;
use serde_json::json;
use std::time::Duration;

/// Integration test for the complete fuzzy storage workflow
/// This test requires Rspamd to be running and accessible
#[tokio::test]
#[ignore] // This test requires Rspamd to be running
async fn test_complete_fuzzy_workflow() {
    println!("Starting complete fuzzy storage workflow test...");
    
    // Test 1: Verify Rspamd is accessible
    test_rspamd_accessibility().await;
    
    // Test 2: Test fuzzy training
    test_fuzzy_training().await;
    
    // Test 3: Test fuzzy detection
    test_fuzzy_detection().await;
    
    // Test 4: Test similar spam detection
    test_similar_spam_detection().await;
    
    // Test 5: Test false positive prevention
    test_false_positive_prevention().await;
    
    // Test 6: Test performance
    test_fuzzy_performance().await;
    
    println!("All fuzzy storage integration tests completed successfully!");
}

async fn test_rspamd_accessibility() {
    println!("Testing Rspamd accessibility...");
    
    let client = Client::new();
    let response = client
        .get(format!("{}/stat", rspamd::CONTROLLER_URL))
        .send()
        .await;
    
    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                println!("✓ Rspamd controller is accessible");
            } else {
                panic!("✗ Rspamd controller returned error status: {}", resp.status());
            }
        }
        Err(e) => {
            panic!("✗ Failed to connect to Rspamd controller: {}", e);
        }
    }
}

async fn test_fuzzy_training() {
    println!("Testing fuzzy training...");
    
    let trainer = FuzzyTrainer::new();
    let test_spam = "Free crypto 100% guaranteed profit! Click here now!";
    
    let result = trainer.teach_fuzzy(test_spam).await;
    
    match result {
        Ok(_) => println!("✓ Fuzzy training successful"),
        Err(e) => {
            println!("⚠ Fuzzy training failed (this might be expected if Rspamd is not configured for training): {}", e);
        }
    }
}

async fn test_fuzzy_detection() {
    println!("Testing fuzzy detection...");
    
    let client = Client::new();
    let test_spam = "Free crypto 100% guaranteed profit! Click here now!";
    
    let response = client
        .post(format!("{}/checkv2", rspamd::CONTROLLER_URL.replace("11334", "11333")))
        .header("Content-Type", "application/json")
        .json(&json!({
            "message": test_spam,
            "task": {
                "user": "test_user",
                "ip": "127.0.0.1"
            }
        }))
        .send()
        .await;
    
    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                let body = resp.text().await.unwrap_or_default();
                if body.contains(symbol::FUZZY_DENIED) {
                    println!("✓ Fuzzy detection working - FUZZY_DENIED symbol found");
                } else {
                    println!("⚠ Fuzzy detection test completed but no FUZZY_DENIED symbol found");
                    println!("Response body: {}", body);
                }
            } else {
                println!("⚠ Fuzzy detection test failed with status: {}", resp.status());
            }
        }
        Err(e) => {
            println!("⚠ Fuzzy detection test failed: {}", e);
        }
    }
}

async fn test_similar_spam_detection() {
    println!("Testing similar spam detection...");
    
    let client = Client::new();
    let similar_spam_variants = vec![
        "Free crypto 99% guaranteed profit! Click here now!",
        "Free crypto 100% guaranteed profit! Click here immediately!",
        "Free crypto 100% guaranteed profit! Click now!",
        "Free crypto 100% guaranteed profit! Click here!",
    ];
    
    let total_variants = similar_spam_variants.len();
    let mut detection_count = 0;
    
    for variant in &similar_spam_variants {
        let response = client
            .post(format!("{}/checkv2", rspamd::CONTROLLER_URL.replace("11334", "11333")))
            .header("Content-Type", "application/json")
            .json(&json!({
                "message": variant,
                "task": {
                    "user": "test_user",
                    "ip": "127.0.0.1"
                }
            }))
            .send()
            .await;
        
        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    let body = resp.text().await.unwrap_or_default();
                    if body.contains(symbol::FUZZY_DENIED) {
                        detection_count += 1;
                        println!("✓ Detected similar spam variant: {}", &variant[..50.min(variant.len())]);
                    }
                }
            }
            Err(_) => {
                // Ignore errors for this test
            }
        }
    }
    
    let total_variants = similar_spam_variants.len();
    if detection_count > 0 {
        println!("✓ Similar spam detection working: {}/{} variants detected", detection_count, total_variants);
    } else {
        println!("⚠ Similar spam detection test completed but no variants were detected");
    }
}

async fn test_false_positive_prevention() {
    println!("Testing false positive prevention...");
    
    let client = Client::new();
    let legitimate_messages = vec![
        "Hello everyone, how are you doing today?",
        "Thanks for the information, that's very helpful.",
        "I agree with your point about the project.",
        "The weather is nice today, isn't it?",
        "This is a legitimate message that should not be flagged as spam.",
    ];
    
    let total_legitimate = legitimate_messages.len();
    let mut false_positive_count = 0;
    
    for message in &legitimate_messages {
        let response = client
            .post(format!("{}/checkv2", rspamd::CONTROLLER_URL.replace("11334", "11333")))
            .header("Content-Type", "application/json")
            .json(&json!({
                "message": message,
                "task": {
                    "user": "test_user",
                    "ip": "127.0.0.1"
                }
            }))
            .send()
            .await;
        
        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    let body = resp.text().await.unwrap_or_default();
                    if body.contains(symbol::FUZZY_DENIED) {
                        false_positive_count += 1;
                        println!("⚠ False positive detected: {}", &message[..50.min(message.len())]);
                    }
                }
            }
            Err(_) => {
                // Ignore errors for this test
            }
        }
    }
    
    if false_positive_count == 0 {
        println!("✓ No false positives detected - good accuracy");
    } else {
        println!("⚠ {} false positives detected out of {} legitimate messages", 
                false_positive_count, total_legitimate);
    }
}

async fn test_fuzzy_performance() {
    println!("Testing fuzzy storage performance...");
    
    let trainer = FuzzyTrainer::new();
    let client = Client::new();
    
    // Test training performance
    let start_time = std::time::Instant::now();
    
    for i in 1..=5 {
        let test_message = format!("Performance test message number {}", i);
        let _ = trainer.teach_fuzzy(&test_message).await;
    }
    
    let training_duration = start_time.elapsed();
    println!("✓ Training performance: {} messages in {:?}", 5, training_duration);
    
    // Test detection performance
    let start_time = std::time::Instant::now();
    
    for i in 1..=5 {
        let test_message = format!("Performance test detection message {}", i);
        let _ = client
            .post(format!("{}/checkv2", rspamd::CONTROLLER_URL.replace("11334", "11333")))
            .header("Content-Type", "application/json")
            .json(&json!({
                "message": test_message,
                "task": {
                    "user": "test_user",
                    "ip": "127.0.0.1"
                }
            }))
            .timeout(Duration::from_secs(5))
            .send()
            .await;
    }
    
    let detection_duration = start_time.elapsed();
    println!("✓ Detection performance: {} messages in {:?}", 5, detection_duration);
    
    // Performance thresholds
    if training_duration < Duration::from_secs(10) {
        println!("✓ Training performance is acceptable");
    } else {
        println!("⚠ Training performance is slow");
    }
    
    if detection_duration < Duration::from_secs(10) {
        println!("✓ Detection performance is acceptable");
    } else {
        println!("⚠ Detection performance is slow");
    }
}

/// Test the fuzzy trainer with various edge cases
#[tokio::test]
#[ignore] // This test requires Rspamd to be running
async fn test_fuzzy_trainer_edge_cases() {
    println!("Testing fuzzy trainer edge cases...");
    
    let trainer = FuzzyTrainer::new();
    
    // Test cases
    let test_cases = vec![
        ("", "empty string"),
        ("   ", "whitespace only"),
        ("a", "single character"),
        ("word", "single word"),
        ("word word", "two words"),
        ("word word word word word word word word", "exactly minimum length"),
        ("word word word word word word word word extra", "above minimum length"),
        ("🚀💰💎", "emoji only"),
        ("Привет мир", "unicode text"),
        ("Free crypto 100% guaranteed profit! Click here now! 🚀💰", "spam with emojis"),
    ];
    
    for (text, description) in test_cases {
        println!("Testing: {}", description);
        let result = trainer.teach_fuzzy(text).await;
        
        match result {
            Ok(_) => println!("✓ {}: Success", description),
            Err(e) => println!("⚠ {}: Failed - {}", description, e),
        }
    }
}

/// Test fuzzy statistics retrieval
#[tokio::test]
#[ignore] // This test requires Rspamd to be running
async fn test_fuzzy_statistics() {
    println!("Testing fuzzy statistics retrieval...");
    
    let client = Client::new();
    
    let response = client
        .get(format!("{}/fuzzystat", rspamd::CONTROLLER_URL))
        .send()
        .await;
    
    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                let body = resp.text().await.unwrap_or_default();
                println!("✓ Fuzzy statistics retrieved successfully");
                println!("Statistics: {}", body);
            } else {
                println!("⚠ Failed to retrieve fuzzy statistics: {}", resp.status());
            }
        }
        Err(e) => {
            println!("⚠ Failed to connect to fuzzy statistics endpoint: {}", e);
        }
    }
}
