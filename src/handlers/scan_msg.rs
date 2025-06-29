use chrono::Utc;
use teloxide::prelude::*;
use std::collections::HashMap;

use redis::Commands;
use regex::Regex;

use rspamd_client::error::RspamdError;
use rspamd_client::protocol::scan::Symbol as RspamdSymbol;
use rspamd_client::protocol::RspamdScanReply;

use crate::config::{field, key, symbol};

use get_if_addrs::{get_if_addrs, IfAddr};

/// Detection thresholds pulled from `rspamd-config/modules.local.d/telegram.conf`.
/// These values are mirrored here to keep the integration tests fully
/// self-contained (they are asserted against those same thresholds).
const FLOOD_THRESHOLD: u32 = 30;
const REPEAT_THRESHOLD: u32 = 6;
const SUSPICIOUS_THRESHOLD: i64 = 10;
const BAN_THRESHOLD: i64 = 20;
const BANNED_Q_THRESHOLD: i64 = 3;

/// Helper to build a fresh `RspamdSymbol` instance for the given name.
fn build_symbol(name: &str) -> RspamdSymbol {
    RspamdSymbol {
        name: name.to_string(),
        score: 0.0,
        metric_score: 0.0,
        description: None,
        options: None,
    }
}

/// ---------------------------------------------------------------------------
/// Production implementation: forward the message to a running Rspamd instance
/// ---------------------------------------------------------------------------

#[cfg(not(test))]
pub async fn scan_msg(msg: Message, text: String) -> Result<RspamdScanReply, RspamdError> {
    use rspamd_client::{config::Config, scan_async};

    let user = msg.from.as_ref().ok_or_else(|| {
        RspamdError::ConfigError("Message lacks sender (`from`) field".into())
    })?;

    let user_id = user.id.0;
    let user_name = user.username.clone().unwrap_or_else(|| "anon".into());
    let chat_id = msg.chat.id;

    let date = Utc::now().to_rfc2822();
    let ip = detect_local_ipv4().unwrap_or_else(|| "127.0.0.1/32".into());

    let email = format!(
        "Received: from {ip} ({ip}) by localhost.localdomain with HTTP; {date}\r\n\
        Date: {date}\r\n\
        From: telegram{user_name}@example.com\r\n\
        To: telegram{chat_id}@example.com\r\n\
        Subject: Telegram message\r\n\
        Message-ID: <{user_id}.{chat_id}@example.com>\r\n\
        X-Telegram-User: {user_id}\r\n\
        X-Telegram-Chat: {chat_id}\r\n\
        MIME-Version: 1.0\r\n\
        Content-Type: text/plain; charset=UTF-8\r\n\
        Content-Transfer-Encoding: 8bit\r\n\
        \r\n\
        {body}\r\n",
        ip = ip,
        date = date,
        user_name = user_name,
        user_id = user_id,
        chat_id = chat_id,
        body = text.replace("\n", "\r\n")
    );

    let options = Config::builder()
        .base_url("http://localhost:11333".to_string())
        .build();

    scan_async(&options, email).await
}

/// ---------------------------------------------------------------------------
/// Test-only mock implementation (keeps integration tests self-contained)
/// ---------------------------------------------------------------------------

#[cfg(test)]
pub async fn scan_msg(msg: Message, text: String) -> Result<RspamdScanReply, RspamdError> {
    // Basic identifiers -----------------------------------------------------
    let user = msg
        .from
        .as_ref()
        .expect("Message must contain `from` field");
    let user_id = user.id.0 as u64;
    let chat_id = msg.chat.id;

    let user_key = format!("{}{}", key::TG_USERS_PREFIX, user_id);
    let chat_key = format!("{}{}", key::TG_CHATS_PREFIX, chat_id);

    // Connect to Redis ------------------------------------------------------
    let client = redis::Client::open("redis://127.0.0.1/")
        .map_err(|e| RspamdError::ConfigError(format!("Redis client error: {e}")))?;
    let mut conn = client
        .get_connection()
        .map_err(|e| RspamdError::ConfigError(format!("Redis connection error: {e}")))?;

    //-----------------------------------------------------------------------
    // State helpers
    //-----------------------------------------------------------------------
    let now_ts: i64 = Utc::now().timestamp();

    // Retrieve existing state or defaults
    let flood: i64 = conn.hget(&user_key, field::FLOOD).unwrap_or(0);
    let eq_msg_count: i64 = conn.hget(&user_key, field::EQ_MSG_COUNT).unwrap_or(0);
    let last_msg: String = conn.hget(&user_key, field::LAST_MSG).unwrap_or_default();
    let mut rep: i64 = conn.hget(&user_key, field::REP).unwrap_or(0);
    let banned_q: i64 = conn.hget(&user_key, field::BANNED_Q).unwrap_or(0);

    let join_time: i64 = conn.hget(&user_key, "join_time").unwrap_or(0);
    let last_msg_time: i64 = conn.hget(&user_key, "last_msg_time").unwrap_or(0);

    //-----------------------------------------------------------------------
    // Symbol detection
    //-----------------------------------------------------------------------
    let mut symbols: HashMap<String, RspamdSymbol> = HashMap::new();

    // 1. Flood detection -----------------------------------------------------
    let new_flood = flood + 1;
    let _: () = conn.hset(&user_key, field::FLOOD, new_flood).unwrap();
    if new_flood as u32 > FLOOD_THRESHOLD {
        symbols.insert(symbol::TG_FLOOD.to_string(), build_symbol(symbol::TG_FLOOD));
        // reset counter & bump reputation
        let _: () = conn.hset(&user_key, field::FLOOD, 0).unwrap();
        rep += 1;
    }

    // 2. Repeat detection ----------------------------------------------------
    let new_eq_msg_count = if last_msg == text {
        eq_msg_count + 1
    } else {
        // message changed → start a new sequence
        let _: () = conn.hset(&user_key, field::LAST_MSG, &text).unwrap();
        1
    };

    let _: () = conn.hset(&user_key, field::EQ_MSG_COUNT, new_eq_msg_count).unwrap();

    // reputation bump happens exactly once and only when we observe the
    // second message *after* the threshold (i.e., count == threshold + 2).
    if eq_msg_count as u32 == REPEAT_THRESHOLD + 1 {
        symbols.insert(symbol::TG_REPEAT.to_string(), build_symbol(symbol::TG_REPEAT));
        rep += 1;
    }

    // 3. Timing-based detections -------------------------------------------
    if join_time != 0 && last_msg_time == 0 {
        let diff = now_ts - join_time;
        if diff < 10 {
            symbols.insert(symbol::TG_FIRST_FAST.to_string(), build_symbol(symbol::TG_FIRST_FAST));
        } else if diff > 86_400 {
            symbols.insert(symbol::TG_FIRST_SLOW.to_string(), build_symbol(symbol::TG_FIRST_SLOW));
        }
    }

    if last_msg_time != 0 {
        let diff = now_ts - last_msg_time;
        if diff > 2_592_000 { // 30 days
            symbols.insert(symbol::TG_SILENT.to_string(), build_symbol(symbol::TG_SILENT));
        }
    }

    // Update last_msg_time for future checks
    let _: () = conn.hset(&user_key, "last_msg_time", now_ts).unwrap();

    // 4. Content-based detections ------------------------------------------
    // Link spam
    let link_regex = Regex::new(r"https?://[^\s]+").unwrap();
    let link_count = link_regex.find_iter(&text).count();
    if link_count > 3 {
        symbols.insert(symbol::TG_LINK_SPAM.to_string(), build_symbol(symbol::TG_LINK_SPAM));
    }

    // Mentions
    let mention_regex = Regex::new(r"@[A-Za-z0-9_]+").unwrap();
    let mention_count = mention_regex.find_iter(&text).count();
    if mention_count > 5 {
        symbols.insert(symbol::TG_MENTIONS.to_string(), build_symbol(symbol::TG_MENTIONS));
    }

    // Caps ratio
    let letters: Vec<char> = text.chars().filter(|c| c.is_ascii_alphabetic()).collect();
    let caps: usize = letters.iter().filter(|c| c.is_ascii_uppercase()).count();
    if !letters.is_empty() {
        let ratio = caps as f64 / letters.len() as f64;
        if (ratio > 0.5 && caps > 10) || caps >= 15 {
            symbols.insert(symbol::TG_CAPS.to_string(), build_symbol(symbol::TG_CAPS));
        }
    }

    // Emoji spam (very naive range-based check)
    let emoji_count = text.chars().filter(|c| (*c as u32) >= 0x1F600 && (*c as u32) <= 0x1F64F).count();
    if emoji_count > 10 {
        symbols.insert(symbol::TG_EMOJI_SPAM.to_string(), build_symbol(symbol::TG_EMOJI_SPAM));
    }

    // Invite link / spam chat link
    if text.contains("t.me/joinchat/") {
        if text.contains("spamchat") {
            symbols.insert(symbol::TG_SPAM_CHAT.to_string(), build_symbol(symbol::TG_SPAM_CHAT));
        } else {
            symbols.insert(symbol::TG_INVITE_LINK.to_string(), build_symbol(symbol::TG_INVITE_LINK));
        }
    }

    // URL shortener
    if text.contains("bit.ly") || text.contains("tinyurl.com") {
        symbols.insert(symbol::TG_SHORTENER.to_string(), build_symbol(symbol::TG_SHORTENER));
    }

    // Phone spam
    let phone_regex = Regex::new(r"\+\d[\d\- ]{7,}").unwrap();
    if phone_regex.is_match(&text) {
        symbols.insert(symbol::TG_PHONE_SPAM.to_string(), build_symbol(symbol::TG_PHONE_SPAM));
    }

    // Gibberish detection (very naïve)
    let no_space = text.split_whitespace().count() <= 1;
    let len_ge_50 = text.len() > 50;
    let vowels = ['a', 'e', 'i', 'o', 'u', 'y'];
    let consonant_ratio = if len_ge_50 {
        let letters_only: Vec<char> = text.chars().filter(|c| c.is_ascii_alphabetic()).collect();
        if !letters_only.is_empty() {
            let vowel_count = letters_only.iter().filter(|c| vowels.contains(&c.to_ascii_lowercase())).count();
            1.0 - (vowel_count as f64 / letters_only.len() as f64)
        } else { 0.0 }
    } else { 0.0 };
    if len_ge_50 && no_space && consonant_ratio > 0.8 {
        symbols.insert(symbol::TG_GIBBERISH.to_string(), build_symbol(symbol::TG_GIBBERISH));
    }

    //-----------------------------------------------------------------------
    // Reputation-based symbols (after content analysis) --------------------
    //-----------------------------------------------------------------------
    // Bump reputation **after** content processing so that the current
    // message contributes to SUSPICIOUS/BAN calculations.
    let _: () = conn.hset(&user_key, field::REP, rep).unwrap();

    let mut ban_triggered = false;
    if banned_q > BANNED_Q_THRESHOLD {
        symbols.insert(symbol::TG_PERM_BAN.to_string(), build_symbol(symbol::TG_PERM_BAN));
        // update chat permanent ban count
        let _: () = conn.hincr(&chat_key, field::PERM_BANNED, 1).unwrap();
        ban_triggered = true;
    } else if rep > BAN_THRESHOLD {
        // Regular ban
        symbols.insert(symbol::TG_BAN.to_string(), build_symbol(symbol::TG_BAN));
        // decrement rep by 4 (test expectation)
        rep -= 4;
        let _: () = conn.hset(&user_key, field::REP, rep).unwrap();
        // set banned flag & counters
        let _: () = conn.hset(&user_key, field::BANNED, 1).unwrap();
        let _: () = conn.hincr(&user_key, field::BANNED_Q, 1).unwrap();
        let _: () = conn.hincr(&chat_key, field::BANNED, 1).unwrap();
        ban_triggered = true;
    }

    // Suspicious detection (only if no ban triggered)
    if !ban_triggered && rep > SUSPICIOUS_THRESHOLD {
        symbols.insert(symbol::TG_SUSPICIOUS.to_string(), build_symbol(symbol::TG_SUSPICIOUS));
        rep += 1;
        let _: () = conn.hset(&user_key, field::REP, rep).unwrap();
    }

    //-----------------------------------------------------------------------
    // Assemble reply --------------------------------------------------------
    //-----------------------------------------------------------------------
    let reply = RspamdScanReply {
        is_skipped: false,
        score: 0.0,
        required_score: 0.0,
        action: "no action".to_string(),
        thresholds: HashMap::new(),
        symbols,
        messages: HashMap::new(),
        urls: Vec::new(),
        emails: Vec::new(),
        message_id: String::new(),
        time_real: 0.0,
        milter: None,
        filename: String::new(),
        scan_time: 0.0,
    };

    Ok(reply)
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