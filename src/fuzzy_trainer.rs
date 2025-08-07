use anyhow::Result;
use reqwest::Client;
use crate::config::rspamd;

/// Handles fuzzy storage training for Rspamd.
/// 
/// This module provides functionality to teach Rspamd's fuzzy storage system
/// about spam messages that have been deleted by the bot. The fuzzy storage
/// uses shingle-based hashing to detect similar spam variants.
pub struct FuzzyTrainer {
    client: Client,
    pub controller_url: String,
    password: String,
}

impl FuzzyTrainer {
    /// Creates a new FuzzyTrainer instance.
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            controller_url: rspamd::CONTROLLER_URL.to_string(),
            password: rspamd::PASSWORD.to_string(),
        }
    }

    /// Teaches the fuzzy storage system about a spam message.
    /// 
    /// This method sends the text content to Rspamd's fuzzy storage
    /// for future detection of similar spam variants.
    /// 
    /// # Arguments
    /// 
    /// * `text` - The text content of the spam message to train on
    /// 
    /// # Returns
    /// 
    /// Returns `Ok(())` if training was successful, or an error if it failed.
    pub async fn teach_fuzzy(&self, text: &str) -> Result<()> {
        // Skip training if text is too short
        if text.trim().split_whitespace().count() < rspamd::MIN_TEXT_LENGTH {
            return Ok(());
        }

        let response = self.client
            .post(format!("{}/fuzzyadd", self.controller_url))
            .header("Password", &self.password)
            .header("Flag", rspamd::FUZZY_FLAG.to_string())
            .header("Weight", rspamd::FUZZY_WEIGHT.to_string())
            .body(text.to_owned())
            .send()
            .await?;

        response.error_for_status()?;
        Ok(())
    }
}
