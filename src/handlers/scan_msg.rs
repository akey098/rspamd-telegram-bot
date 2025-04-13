use rspamd_client::{config::Config, error::RspamdError, protocol::RspamdScanReply, scan_async};
use teloxide::prelude::*;

pub async fn scan_msg(msg: Message, text: String) -> Result<RspamdScanReply, RspamdError> {
    let user = msg.from.unwrap();
    let user_id = user.id.to_string();
    let text = text;
    let email = format!(
        "From: telegram{}@local\nSubject: Telegram message\nX-Telegram-User: {}\n\n{}",
        user_id, user_id, text
    );
    let options = Config::builder()
        .base_url("http://localhost:11333".to_string())
        .build();
    scan_async(&options, email).await
}
