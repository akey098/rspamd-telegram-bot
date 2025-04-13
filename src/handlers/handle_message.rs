use crate::handlers::scan_msg;
use std::error::Error;
use teloxide::prelude::*;

pub async fn handle_message(
    bot: Bot,
    message: Message,
) -> Result<(), Box<dyn Error + Send + Sync>> {

    let text = if let Some(text) = message.text() {
        text.to_string()
    } else {
        return Ok(());
    };
    let result =  scan_msg(message.clone(), text).await;

    let scan_result = result.ok().unwrap();


    if scan_result.score >= 10.0 || scan_result.symbols.contains_key("TG_FLOOD") || scan_result.symbols.contains_key("TG_SUSPICIOUS") {
        // For instance, delete the message using your async Telegram API client.
        println!("Deleting message {} from chat {} because it appears to be spam.", message.id, message.chat.id);
        bot.delete_message(message.chat.id, message.id).await?;
    } else if scan_result.score >= 5.0 {
        // Optionally, warn the user.
        println!("Warning user {} in chat {} about spammy behavior.", message.from.unwrap().id, message.chat.id);
        bot.send_message(message.chat.id, "You are behaving spammy. Further actions will result ban.").await?;
    } else {
        println!("Message is ok")
    }

    Ok(())
}
