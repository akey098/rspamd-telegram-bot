use chrono::Utc;
use rspamd_client::{config::Config, error::RspamdError, protocol::RspamdScanReply, scan_async};
use teloxide::prelude::*;
use crate::utils;

pub async fn scan_msg(msg: Message, text: String) -> Result<RspamdScanReply, RspamdError> {
    let user = msg.from.unwrap();
    let user_id = user.id.to_string();
    let user_name = user.username.expect("REASON").to_string();
    let chat_id = msg.chat.id;
    let chat_name = msg.chat.title().unwrap().to_string();
    let date = Utc::now().to_rfc2822();
    let text = text;
    let ip = utils::detect_local_ipv4().unwrap().to_string();
    let email = format!(
        "Received: from {ip} ({ip}) by localhost.localdomain with HTTP; {date}\r\n\
        Date: {date}\r\n\
        From: telegram{user_name}@example.com\r\n\
        To: telegram{chat_name}@example.com\r\n\
        Subject: Telegram message\r\n\
        Message-ID: <{user_id}.{chat_id}@example.com>\r\n\
        X-Telegram-User: {user_id}\r\n\
        MIME-Version: 1.0\r\n\
        Content-Type: text/plain; charset=UTF-8\r\n\
        Content-Transfer-Encoding: 8bit\r\n\
        \r\n\
        {text}\r\n",
        date = date,
        ip = ip,
        user_name = user_name,
        chat_name = chat_name,
        user_id = user_id,
        chat_id = chat_id,
        text = text.replace("\n", "\r\n")
    );
    let options = Config::builder()
        .base_url("http://localhost:11333".to_string())
        .build();
    scan_async(&options, email).await
}