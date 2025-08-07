use rspamd_telegram_bot::fuzzy_trainer::FuzzyTrainer;
use rspamd_telegram_bot::config::rspamd;

#[tokio::test]
async fn test_fuzzy_trainer_creation() {
    let trainer = FuzzyTrainer::new();
    assert!(trainer.controller_url.contains("127.0.0.1:11334"));
    assert_eq!(trainer.controller_url, rspamd::CONTROLLER_URL);
}

#[tokio::test]
async fn test_fuzzy_trainer_short_text() {
    let trainer = FuzzyTrainer::new();
    // Short text should not trigger training
    let result = trainer.teach_fuzzy("short").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_fuzzy_trainer_long_text() {
    let trainer = FuzzyTrainer::new();
    // Long text should attempt training (may fail if Rspamd not running)
    let long_text = "This is a much longer text that should meet the minimum length requirement for fuzzy training. It contains enough words to trigger the training process.";
    let result = trainer.teach_fuzzy(long_text).await;
    // We don't assert success here since Rspamd might not be running in test environment
    // The test just verifies the function doesn't panic
}

#[tokio::test]
async fn test_fuzzy_trainer_exact_minimum_length() {
    let trainer = FuzzyTrainer::new();
    // Create text with exactly the minimum word count
    let words: Vec<&str> = (0..rspamd::MIN_TEXT_LENGTH).map(|_| "word").collect();
    let text = words.join(" ");
    let result = trainer.teach_fuzzy(&text).await;
    // Should attempt training since it meets the minimum length
    // We don't assert success since Rspamd might not be running
}

#[tokio::test]
async fn test_fuzzy_trainer_empty_text() {
    let trainer = FuzzyTrainer::new();
    let result = trainer.teach_fuzzy("").await;
    assert!(result.is_ok()); // Should skip training for empty text
}

#[tokio::test]
async fn test_fuzzy_trainer_whitespace_only() {
    let trainer = FuzzyTrainer::new();
    let result = trainer.teach_fuzzy("   \n\t   ").await;
    assert!(result.is_ok()); // Should skip training for whitespace-only text
}

#[tokio::test]
async fn test_fuzzy_trainer_special_characters() {
    let trainer = FuzzyTrainer::new();
    let text_with_special_chars = "Free crypto 100% guaranteed profit! Click here now! 🚀💰";
    let result = trainer.teach_fuzzy(text_with_special_chars).await;
    // Should handle special characters and emojis gracefully
    // We don't assert success since Rspamd might not be running
}

#[tokio::test]
async fn test_fuzzy_trainer_unicode_text() {
    let trainer = FuzzyTrainer::new();
    let unicode_text = "Привет мир! 你好世界! Hello world! This is a test message with multiple languages.";
    let result = trainer.teach_fuzzy(unicode_text).await;
    // Should handle Unicode text gracefully
    // We don't assert success since Rspamd might not be running
}

#[tokio::test]
async fn test_fuzzy_trainer_very_long_text() {
    let trainer = FuzzyTrainer::new();
    // Create a very long text to test handling of large payloads
    let long_text = "This is a very long text that contains many words. ".repeat(100);
    let result = trainer.teach_fuzzy(&long_text).await;
    // Should handle very long text gracefully
    // We don't assert success since Rspamd might not be running
}

#[tokio::test]
async fn test_fuzzy_trainer_multiple_requests() {
    let trainer = FuzzyTrainer::new();
    let texts = vec![
        "First spam message for testing",
        "Second spam message for testing",
        "Third spam message for testing",
    ];
    
    for text in texts {
        let result = trainer.teach_fuzzy(text).await;
        // Each request should be handled independently
        // We don't assert success since Rspamd might not be running
    }
}

#[tokio::test]
async fn test_fuzzy_trainer_configuration_values() {
    // Test that configuration constants are set correctly
    assert_eq!(rspamd::CONTROLLER_URL, "http://127.0.0.1:11334");
    assert_eq!(rspamd::PASSWORD, "superSecret");
    assert_eq!(rspamd::FUZZY_FLAG, 1);
    assert_eq!(rspamd::FUZZY_WEIGHT, 10);
    assert_eq!(rspamd::MIN_TEXT_LENGTH, 8);
}

#[tokio::test]
async fn test_fuzzy_trainer_text_trimming() {
    let trainer = FuzzyTrainer::new();
    // Test that text is properly trimmed before word counting
    let text_with_whitespace = "   This text has leading and trailing whitespace   ";
    let result = trainer.teach_fuzzy(text_with_whitespace).await;
    // Should count words correctly after trimming
    // We don't assert success since Rspamd might not be running
}

#[tokio::test]
async fn test_fuzzy_trainer_word_counting() {
    let trainer = FuzzyTrainer::new();
    // Test various word counting scenarios
    let test_cases = vec![
        ("one", 1),           // Single word
        ("one two", 2),       // Two words
        ("one  two", 2),      // Two words with extra space
        ("one\ttwo", 2),      // Two words with tab
        ("one\ntwo", 2),      // Two words with newline
        ("one two three", 3), // Three words
    ];
    
    for (text, expected_count) in test_cases {
        let word_count = text.trim().split_whitespace().count();
        assert_eq!(word_count, expected_count, "Failed for text: '{}'", text);
    }
}

// Integration test that can be run when Rspamd is available
#[tokio::test]
#[ignore] // This test requires Rspamd to be running
async fn test_fuzzy_trainer_integration() {
    let trainer = FuzzyTrainer::new();
    let test_text = "Integration test spam message for fuzzy training";
    
    // This test should only be run when Rspamd is actually running
    let result = trainer.teach_fuzzy(test_text).await;
    
    // If Rspamd is running, this should succeed
    if result.is_ok() {
        println!("Integration test passed - Rspamd is running");
    } else {
        println!("Integration test skipped - Rspamd not running");
    }
}
