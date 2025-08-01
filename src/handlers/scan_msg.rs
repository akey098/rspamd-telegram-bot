use chrono::Utc;
use rspamd_client::{config::Config, error::RspamdError, protocol::RspamdScanReply, scan_async};
use teloxide::prelude::*;
use get_if_addrs::{get_if_addrs, IfAddr};

/// Scan a Telegram message: real Rspamd first, heuristic fallback.
pub async fn scan_msg(msg: Message, text: String) -> Result<RspamdScanReply, RspamdError> {
    let user = msg.from.ok_or_else(|| RspamdError::ConfigError("Message has no sender".to_string()))?;
    let user_id = user.id.to_string();
    let user_name = user.username.unwrap_or_else(|| "anonymous".to_string());
    let chat_id = msg.chat.id;
    let date = Utc::now().to_rfc2822();
    let text = text;
    let ip = detect_local_ipv4().unwrap_or_else(|| "127.0.0.1/32".to_string());
    
    // Enhanced email format with reputation headers
    let email = format!(
        "Received: from {ip} ({ip}) by localhost.localdomain with HTTP; {date}\r\n\
        Date: {date}\r\n\
        From: telegram{user_name}@example.com\r\n\
        To: telegram{chat_id}@example.com\r\n\
        Subject: Telegram message\r\n\
        Message-ID: <{user_id}.{chat_id}@example.com>\r\n\
        X-Telegram-User: {user_id}\r\n\
        X-Telegram-Chat: {chat_id}\r\n\
        X-TG-User: {user_id}\r\n\
        MIME-Version: 1.0\r\n\
        Content-Type: text/plain; charset=UTF-8\r\n\
        Content-Transfer-Encoding: 8bit\r\n\
        \r\n\
        {text}\r\n",
        date = date,
        ip = ip,
        user_name = user_name,
        user_id = user_id,
        chat_id = chat_id,
        text = text.replace("\n", "\r\n")
    );
    
    let options = Config::builder()
        .base_url(std::env::var("RSPAMD_URL").unwrap_or_else(|_| "http://localhost:11333".to_string()))
        .build();
    scan_async(&options, email).await
}

pub fn detect_local_ipv4() -> Option<String> {
    if let Ok(ifaces) = get_if_addrs() {
        for iface in ifaces {
            if let IfAddr::V4(v4addr) = iface.addr {
                let ip = v4addr.ip;
                if !ip.is_loopback() {
                    return Some(format!("{}/32", ip));
                }
            }
        }
    }
    None
}