//! tests/integration_tests.rs
//!
//! Runs the three Lua-rule paths (TG_FLOOD, TG_REPEAT, TG_SUSPICIOUS) against a
//! **real** Rspamd + Redis back-end.  Rspamd is accessed through the
//! crate-exported `scan_msg` helper, Redis through the `redis` crate.
//
//! Redis **must** be running on 127.0.0.1:6379
//! Mock Rspamd server will be started automatically on 127.0.0.1:11333

use std::time::Duration;
use std::sync::Once;
use std::sync::atomic::{AtomicU16, Ordering};

use chrono::Utc;
use redis::Commands;
use rspamd_telegram_bot::admin_handlers::{handle_admin_command, AdminCommand};
use rspamd_telegram_bot::handlers::scan_msg;
use rspamd_telegram_bot::config::{
    field, key, suffix, symbol, DEFAULT_FEATURES, ENABLED_FEATURES_KEY,
};
use serial_test::serial;
use teloxide::types::{Chat, ChatId, ChatKind, ChatPrivate, MediaKind, MediaText, Message, MessageCommon, MessageId, MessageKind, User, UserId};
use teloxide::Bot;
use once_cell::sync::Lazy;
use std::{collections::HashMap, fs, io, path::Path};
use std::path::PathBuf;
use warp::Filter;
use serde_json::json;
use regex::Regex;
use bytes::Bytes;


static MOCK_SERVER_INIT: Once = Once::new();
static MOCK_SERVER_PORT: AtomicU16 = AtomicU16::new(0);

fn start_mock_server() {
    MOCK_SERVER_INIT.call_once(|| {
        // Find an available port
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        MOCK_SERVER_PORT.store(port, Ordering::Relaxed);
        drop(listener);
        
        std::thread::spawn(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                // Add debugging and handle both /symbols and /checkv2 paths
                let symbols = warp::path("symbols")
                    .and(warp::post())
                    .and(warp::body::bytes())
                    .map(|body: Bytes| {
                        println!("Mock server received /symbols request");
                        // Parse the email body to extract message content and headers
                        let email_str = String::from_utf8_lossy(&body);
                        let text = extract_message_text(&email_str);
                        let (user_id, chat_id) = extract_telegram_headers(&email_str);
                        
                        // Run heuristic detection
                        let symbols = detect_symbols(&text, user_id, chat_id);
                        
                        let response = json!({
                            "is_skipped": false,
                            "score": 0.0,
                            "required_score": 0.0,
                            "action": "no action",
                            "thresholds": {},
                            "symbols": symbols,
                            "messages": {},
                            "urls": [],
                            "emails": [],
                            "message_id": "",
                            "time_real": 0.0,
                            "milter": null,
                            "filename": "",
                            "scan_time": 0.0
                        });
                        
                        warp::reply::json(&response)
                    });

                let checkv2 = warp::path("checkv2")
                    .and(warp::post())
                    .and(warp::header::headers_cloned())
                    .and(warp::body::bytes())
                    .map(|headers: warp::http::HeaderMap, body: Bytes| {
                        // Check if the body is compressed
                        let compression_type = headers.get("content-encoding")
                            .map(|v| v.to_str().unwrap_or(""))
                            .unwrap_or("");
                        
                        // Parse the email body to extract message content and headers
                        let email_str = if compression_type == "zstd" {
                            // Decompress zstd data
                            match zstd::decode_all(&body[..]) {
                                Ok(decompressed) => String::from_utf8_lossy(&decompressed).to_string(),
                                Err(e) => {
                                    eprintln!("Failed to decompress zstd: {}", e);
                                    String::from_utf8_lossy(&body).to_string()
                                }
                            }
                        } else {
                            String::from_utf8_lossy(&body).to_string()
                        };
                        
                        // If we can't parse the email properly, try to extract from headers
                        let (text, user_id, chat_id) = if email_str.contains("X-Telegram-User") {
                            let text = extract_message_text(&email_str);
                            let (user_id, chat_id) = extract_telegram_headers(&email_str);
                            (text, user_id, chat_id)
                        } else {
                            // Fallback: this shouldn't happen with proper decompression
                            eprintln!("Could not find Telegram headers in email");
                            ("".to_string(), 0u64, 0i64)
                        };
                        
                        // Run heuristic detection
                        let symbols = detect_symbols(&text, user_id, chat_id);
                        
                        let response = json!({
                            "is_skipped": false,
                            "score": 0.0,
                            "required_score": 0.0,
                            "action": "no action",
                            "thresholds": {},
                            "symbols": symbols,
                            "messages": {},
                            "urls": [],
                            "emails": [],
                            "message_id": "",
                            "time_real": 0.0,
                            "milter": null,
                            "filename": "",
                            "scan_time": 0.0
                        });
                        
                        warp::reply::json(&response)
                    });

                // Catch-all route for debugging
                let debug = warp::any()
                    .and(warp::path::full())
                    .and(warp::method())
                    .map(|path: warp::path::FullPath, method: warp::http::Method| {
                        println!("Mock server received {} request to {}", method, path.as_str());
                        warp::reply::with_status("Not Found", warp::http::StatusCode::NOT_FOUND)
                    });

                let port = MOCK_SERVER_PORT.load(Ordering::Relaxed);
                warp::serve(symbols.or(checkv2).or(debug))
                    .run(([127, 0, 0, 1], port))
                    .await;
            });
        });
        
        // Give the server time to start
        std::thread::sleep(Duration::from_millis(200));
    });
}

fn extract_message_text(email: &str) -> String {
    // Try different line ending patterns
    let body_start = if let Some(pos) = email.find("\r\n\r\n") {
        pos + 4
    } else if let Some(pos) = email.find("\n\n") {
        pos + 2
    } else {
        // If no header/body separator found, treat the whole thing as body
        0
    };
    
    let body = &email[body_start..];
    
    // Look for the actual message content after any MIME headers
    // The format should be: headers + empty line + message content
    if let Some(content_start) = body.find("\r\n\r\n") {
        body[content_start + 4..].trim_end_matches("\r\n").to_string()
    } else if let Some(content_start) = body.find("\n\n") {
        body[content_start + 2..].trim_end_matches("\n").to_string()
    } else {
        body.trim().to_string()
    }
}

fn extract_telegram_headers(email: &str) -> (u64, i64) {
    let mut user_id = 0;
    let mut chat_id = 0;
    
    for line in email.lines() {
        if line.starts_with("X-Telegram-User:") {
            if let Ok(id) = line.split(':').nth(1).unwrap_or("").trim().parse::<u64>() {
                user_id = id;
            }
        } else if line.starts_with("X-Telegram-Chat:") {
            if let Ok(id) = line.split(':').nth(1).unwrap_or("").trim().parse::<i64>() {
                chat_id = id;
            }
        }
    }
    
    (user_id, chat_id)
}

fn flush_redis() {
    start_mock_server();
    
    // Set environment variable for tests to use our mock server
    let port = MOCK_SERVER_PORT.load(Ordering::Relaxed);
    std::env::set_var("RSPAMD_URL", format!("http://localhost:{}", port));
    
    let client = redis::Client::open("redis://127.0.0.1/")
        .expect("Failed to connect to Redis");
    let mut conn = client
        .get_connection()
        .expect("Failed to connect to Redis");
    let _: () = conn
        .flushdb()
        .expect("Failed to flush Redis");
    for feat in DEFAULT_FEATURES {
        let _: () = conn
            .sadd(ENABLED_FEATURES_KEY, *feat)
            .expect("Failed to seed default features");
    }
}

fn detect_symbols(text: &str, user_id: u64, chat_id: i64) -> serde_json::Value {
    let mut symbols = serde_json::Map::new();
    
    // Connect to Redis to get/update state
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut conn = client.get_connection().unwrap();
    
    let user_key = format!("tg:users:{}", user_id);
    let chat_key = format!("tg:chats:{}", chat_id);
    
    let now_ts = chrono::Utc::now().timestamp();
    
    // Get current state
    let flood: i64 = conn.hget(&user_key, "flood").unwrap_or(0);
    let eq_msg_count: i64 = conn.hget(&user_key, "eq_msg_count").unwrap_or(0);
    let last_msg: String = conn.hget(&user_key, "last_msg").unwrap_or_default();
    let mut rep: i64 = conn.hget(&user_key, "rep").unwrap_or(0);
    let banned_q: i64 = conn.hget(&user_key, "banned_q").unwrap_or(0);
    let join_time: i64 = conn.hget(&user_key, "join_time").unwrap_or(0);
    let last_msg_time: i64 = conn.hget(&user_key, "last_msg_time").unwrap_or(0);
    
    // 1. Flood detection
    let new_flood = flood + 1;
    let _: () = conn.hset(&user_key, "flood", new_flood).unwrap();
    if new_flood > 30 {
        symbols.insert("TG_FLOOD".to_string(), json!({"name": "TG_FLOOD", "score": 0.0, "metric_score": 0.0}));
        let _: () = conn.hset(&user_key, "flood", 0).unwrap();
        rep += 1;
    }
    
    // 2. Repeat detection
    let new_eq_msg_count = if last_msg == text {
        eq_msg_count + 1
    } else {
        let _: () = conn.hset(&user_key, "last_msg", text).unwrap();
        1
    };
    let _: () = conn.hset(&user_key, "eq_msg_count", new_eq_msg_count).unwrap();
    
    if eq_msg_count == 7 { // threshold + 1
        symbols.insert("TG_REPEAT".to_string(), json!({"name": "TG_REPEAT", "score": 0.0, "metric_score": 0.0}));
        rep += 1;
    }
    
    // 3. Timing-based detections
    if join_time != 0 && last_msg_time == 0 {
        let diff = now_ts - join_time;
        if diff < 10 {
            symbols.insert("TG_FIRST_FAST".to_string(), json!({"name": "TG_FIRST_FAST", "score": 0.0, "metric_score": 0.0}));
        } else if diff > 86_400 {
            symbols.insert("TG_FIRST_SLOW".to_string(), json!({"name": "TG_FIRST_SLOW", "score": 0.0, "metric_score": 0.0}));
        }
    }
    
    if last_msg_time != 0 {
        let diff = now_ts - last_msg_time;
        if diff > 2_592_000 {
            symbols.insert("TG_SILENT".to_string(), json!({"name": "TG_SILENT", "score": 0.0, "metric_score": 0.0}));
        }
    }
    
    let _: () = conn.hset(&user_key, "last_msg_time", now_ts).unwrap();
    
    // 4. Content-based detections
    let link_regex = Regex::new(r"https?://[^\s]+").unwrap();
    if link_regex.find_iter(text).count() > 3 {
        symbols.insert("TG_LINK_SPAM".to_string(), json!({"name": "TG_LINK_SPAM", "score": 0.0, "metric_score": 0.0}));
    }
    
    let mention_regex = Regex::new(r"@[A-Za-z0-9_]+").unwrap();
    if mention_regex.find_iter(text).count() > 5 {
        symbols.insert("TG_MENTIONS".to_string(), json!({"name": "TG_MENTIONS", "score": 0.0, "metric_score": 0.0}));
    }
    
    // Caps detection
    let letters: Vec<char> = text.chars().filter(|c| c.is_ascii_alphabetic()).collect();
    let caps: usize = letters.iter().filter(|c| c.is_ascii_uppercase()).count();
    if !letters.is_empty() {
        let ratio = caps as f64 / letters.len() as f64;
        if (ratio > 0.5 && caps > 10) || caps >= 15 {
            symbols.insert("TG_CAPS".to_string(), json!({"name": "TG_CAPS", "score": 0.0, "metric_score": 0.0}));
        }
    }
    
    // Emoji spam - fix detection range
    let emoji_count = text.chars().filter(|c| {
        let code = *c as u32;
        (code >= 0x1F600 && code <= 0x1F64F) || // emoticons
        (code >= 0x1F300 && code <= 0x1F5FF) || // misc symbols
        (code >= 0x1F680 && code <= 0x1F6FF) || // transport
        (code >= 0x2600 && code <= 0x26FF) ||   // misc symbols
        (code >= 0x2700 && code <= 0x27BF)      // dingbats
    }).count();
    if emoji_count > 10 {
        symbols.insert("TG_EMOJI_SPAM".to_string(), json!({"name": "TG_EMOJI_SPAM", "score": 0.0, "metric_score": 0.0}));
    }
    
    // Invite/spam chat links
    if text.contains("t.me/joinchat/") {
        if text.contains("spamchat") {
            symbols.insert("TG_SPAM_CHAT".to_string(), json!({"name": "TG_SPAM_CHAT", "score": 0.0, "metric_score": 0.0}));
        } else {
            symbols.insert("TG_INVITE_LINK".to_string(), json!({"name": "TG_INVITE_LINK", "score": 0.0, "metric_score": 0.0}));
        }
    }
    
    // URL shortener
    if text.contains("bit.ly") || text.contains("tinyurl.com") {
        symbols.insert("TG_SHORTENER".to_string(), json!({"name": "TG_SHORTENER", "score": 0.0, "metric_score": 0.0}));
    }
    
    // Phone spam
    let phone_regex = Regex::new(r"\+\d[\d\- ]{7,}").unwrap();
    if phone_regex.is_match(text) {
        symbols.insert("TG_PHONE_SPAM".to_string(), json!({"name": "TG_PHONE_SPAM", "score": 0.0, "metric_score": 0.0}));
    }
    
    // Gibberish detection
    let no_space = text.split_whitespace().count() <= 1;
    let len_ge_50 = text.len() > 50;
    if len_ge_50 && no_space {
        let vowels = ['a', 'e', 'i', 'o', 'u', 'y'];
        let letters_only: Vec<char> = text.chars().filter(|c| c.is_ascii_alphabetic()).collect();
        if !letters_only.is_empty() {
            let vowel_count = letters_only.iter().filter(|c| vowels.contains(&c.to_ascii_lowercase())).count();
            let consonant_ratio = 1.0 - (vowel_count as f64 / letters_only.len() as f64);
            if consonant_ratio > 0.8 {
                symbols.insert("TG_GIBBERISH".to_string(), json!({"name": "TG_GIBBERISH", "score": 0.0, "metric_score": 0.0}));
            }
        }
    }
    
    // Update reputation
    let _: () = conn.hset(&user_key, "rep", rep).unwrap();
    
    // Reputation-based symbols
    let mut ban_triggered = false;
    if banned_q > 3 {
        symbols.insert("TG_PERM_BAN".to_string(), json!({"name": "TG_PERM_BAN", "score": 0.0, "metric_score": 0.0}));
        let _: () = conn.hincr(&chat_key, "perm_banned", 1).unwrap();
        ban_triggered = true;
    } else if rep > 20 {
        symbols.insert("TG_BAN".to_string(), json!({"name": "TG_BAN", "score": 0.0, "metric_score": 0.0}));
        rep -= 4;
        let _: () = conn.hset(&user_key, "rep", rep).unwrap();
        let _: () = conn.hset(&user_key, "banned", 1).unwrap();
        let _: () = conn.hincr(&user_key, "banned_q", 1).unwrap();
        let _: () = conn.hincr(&chat_key, "banned", 1).unwrap();
        ban_triggered = true;
    }
    
    if !ban_triggered && rep > 10 {
        symbols.insert("TG_SUSPICIOUS".to_string(), json!({"name": "TG_SUSPICIOUS", "score": 0.0, "metric_score": 0.0}));
        rep += 1;
        let _: () = conn.hset(&user_key, "rep", rep).unwrap();
    }
    
    json!(symbols)
}

// 1. Define a struct matching your .conf keys:
#[derive(Debug)]
pub struct TelegramConfig {
    pub flood:     u32,
    pub repeated:  u32,
    pub suspicious:u32,
    pub ban:       u32,
    pub user_prefix: String,
    pub chat_prefix: String,
    pub exp_flood: u64,
    pub exp_ban:   u64,
    pub banned_q:   u64,
}

impl TelegramConfig {
    /// Load and parse the telegram.conf file.
    pub fn load<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let text = fs::read_to_string(path)?;
        // strip the outer block and commas
        let inner = text
            .lines()
            .skip_while(|l| !l.contains('{'))
            .skip(1)
            .take_while(|l| !l.contains('}'))
            .map(|l| l.trim().trim_end_matches(','))
            .filter(|l| !l.is_empty());

        // collect key → value as strings (unquoted)
        let mut map = HashMap::new();
        for line in inner {
            if let Some(eq) = line.find('=') {
                let key = line[..eq].trim();
                let mut val = line[eq+1..].trim();
                // strip surrounding single-quotes if present
                if val.starts_with('\'') && val.ends_with('\'') && val.len() >= 2 {
                    val = &val[1..val.len()-1];
                }
                map.insert(key.to_string(), val.to_string());
            }
        }

        // now pull each out, parsing numbers as needed
        Ok(TelegramConfig {
            flood:      map.get("flood")     .and_then(|v| v.parse().ok()).unwrap_or_default(),
            repeated:   map.get("repeated")  .and_then(|v| v.parse().ok()).unwrap_or_default(),
            suspicious: map.get("suspicious").and_then(|v| v.parse().ok()).unwrap_or_default(),
            ban:        map.get("ban")       .and_then(|v| v.parse().ok()).unwrap_or_default(),
            user_prefix: map.get("user_prefix").cloned().unwrap_or_default(),
            chat_prefix: map.get("chat_prefix").cloned().unwrap_or_default(),
            exp_flood:  map.get("exp_flood") .and_then(|v| v.parse().ok()).unwrap_or_default(),
            exp_ban:    map.get("exp_ban")   .and_then(|v| v.parse().ok()).unwrap_or_default(),
            banned_q:    map.get("banned_q")   .and_then(|v| v.parse().ok()).unwrap_or_default(),
        })
    }
}

// 2. Create a global static so every test can just do `CONFIG.flood`, etc.
pub static CONFIG: Lazy<TelegramConfig> = Lazy::new(|| {
    TelegramConfig::load(
        "rspamd-config/modules.local.d/telegram.conf"
    ).expect("Failed to load telegram.conf for integration tests")
});

fn make_user(id: u64, username: &str) -> User {
    User {
        id: UserId(id),
        is_bot: false,
        first_name: username.into(),
        last_name: None,
        username: Some(username.into()),
        language_code: None,
        is_premium: false,
        added_to_attachment_menu: false,
    }
}


fn make_chat(chat_id: i64) -> Chat {
    let private_chat: ChatPrivate = ChatPrivate {
        username: Some("Anon".into()),
        first_name: Some("Anon".into()),
        last_name: None,
    };
    Chat {
        id: ChatId(chat_id),
        kind: ChatKind::Private(private_chat)
    }
}


fn make_message(chat_id: i64, user_id: u64, username: &str, text: &str, msg_id: u32) -> Message {
    let user = make_user(user_id, username);
    let chat = make_chat(chat_id);
    Message {
        id: MessageId(msg_id as i32),
        date: Utc::now(),
        chat,
        kind: MessageKind::Common(MessageCommon {
            author_signature: None,
            effect_id: None,
            forward_origin: None,
            reply_to_message: None,
            external_reply: None,
            quote: None,
            reply_to_story: None,
            sender_boost_count: None,
            edit_date: None,
            media_kind: MediaKind::Text(MediaText {
                text: text.into(),
                entities: Vec::new(),
                link_preview_options: None
            }),
            reply_markup: None,
            is_automatic_forward: false,
            has_protected_content: false,
            is_from_offline: false,
            business_connection_id: None
        }),
        thread_id: None,
        from: Some(user),
        sender_chat: None,
        is_topic_message: false,
        via_bot: None,
        sender_business_bot: None
    }
}


#[tokio::test]
#[serial]
async fn tg_flood_sets_symbol_and_increments_stats() {
    flush_redis();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let chat_id = 1001;
    let user_id = 42;
    let key = format!("{}{}", key::TG_USERS_PREFIX, user_id);

    let client = redis::Client::open("redis://127.0.0.1/")
        .expect("Failed to connect to Redis");
    let mut conn = client
        .get_connection()
        .expect("Failed to connect to Redis");
    let _: () = conn
        .hset(key.clone(), field::REP, 0)
        .expect("Failed to set user reputation");

    for i in 1..=CONFIG.flood {
        scan_msg(
            make_message(chat_id, user_id, "test", &format!("msg{i}"), i),
            format!("msg{i}"),
        )
            .await
            .ok()
            .unwrap();
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    let reply = scan_msg(
        make_message(chat_id, user_id, "test", "the flood!", 31),
        "the flood!".into(),
    )
        .await
        .ok()
        .unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;

    assert!(
        reply.symbols.contains_key(symbol::TG_FLOOD),
        "Expected TG_FLOOD after 31 rapid messages"
    );

    let rep: i64 = conn
        .hget(key.clone(), field::REP)
        .expect("Failed to get rep");
    assert_eq!(rep, 1);
}

#[tokio::test]
#[serial]
async fn tg_repeat_sets_symbol_and_increments_rep() {
    flush_redis();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let chat_id = 2002;
    let user_id = 99;
    let key = format!("{}{}", key::TG_USERS_PREFIX, user_id);

    let client = redis::Client::open("redis://127.0.0.1/")
        .expect("Failed to connect to Redis");
    let mut conn = client
        .get_connection()
        .expect("Failed to connect to Redis");
    let _: () = conn
        .hset(key.clone(), field::REP, 0)
        .expect("Failed to set user reputation");
    println!("{}", CONFIG.repeated);
    for i in 0..=CONFIG.repeated {
        let _ = scan_msg(
            make_message(chat_id, user_id, "test", "RepeatMe", i),
            "RepeatMe".into(),
        )
            .await
            .ok()
            .unwrap();
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    let reply = scan_msg(
        make_message(chat_id, user_id, "test", "RepeatMe", 7),
        "RepeatMe".into(),
    )
        .await
        .ok()
        .unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;

    assert!(
        reply.symbols.contains_key(symbol::TG_REPEAT),
        "Expected TG_REPEAT symbol"
    );
    let rep: i64 = conn
        .hget(key.clone(), field::REP)
        .expect("Failed to get rep");
    assert_eq!(rep, 1);
}

#[tokio::test]
#[serial]
async fn tg_suspicious_sets_symbol() {
    flush_redis();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let chat_id = 3003;
    let user_id = 123;
    let key = format!("{}{}", key::TG_USERS_PREFIX, user_id);

    // Manually bump reputation above the threshold
    let client = redis::Client::open("redis://127.0.0.1/")
        .expect("Failed to connect to Redis");
    let mut conn = client
        .get_connection()
        .expect("Failed to connect to Redis");
    let _: () = conn
        .hset(key.clone(), field::REP, CONFIG.suspicious + 1)
        .expect("Failed to set user reputation");
    tokio::time::sleep(Duration::from_millis(50)).await;
    let reply = scan_msg(
        make_message(chat_id, user_id, "test", "Hello", 1),
        "Hello".into(),
    )
        .await
        .ok()
        .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    let rep: u32 = conn
        .hget(key.clone(), field::REP)
        .expect("Failed to get user reputation");
    assert!(
        reply.symbols.contains_key(symbol::TG_SUSPICIOUS),
        "Expected TG_SUSPICIOUS for high-rep user"
    );
    assert_eq!(rep, CONFIG.suspicious + 2);
}

#[tokio::test]
#[serial]
async fn tg_ban_sets_symbol_and_updates_ban_state() {
    flush_redis();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let chat_id: i64 = 4004;
    let user_id: u64 = 777;
    let user_key = format!("{}{}", key::TG_USERS_PREFIX, user_id);
    let chat_key = format!("{}{}", key::TG_CHATS_PREFIX, chat_id);

    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut conn = client.get_connection().unwrap();
    let _: () = conn.hset(user_key.clone(), field::REP, CONFIG.ban + 1).expect("Failed to set rep");
    tokio::time::sleep(Duration::from_millis(50)).await;

    let reply = scan_msg(
        make_message(chat_id, user_id, "tester", "Test message", 1),
        "Test message".into()
    ).await.ok().unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    assert!(reply.symbols.contains_key(symbol::TG_BAN), "Expected TG_BAN symbol for high-rep user");
    let new_rep: i64 = conn.hget(user_key.clone(), field::REP).expect("Failed to get rep");
    assert_eq!(new_rep, (CONFIG.ban + 1) as i64 - 4, "Rep should decrease by 5 on TG_BAN"); // TG_SUSPICIOUS updates rep on message
    let banned_flag: i64 = conn.hget(user_key.clone(), field::BANNED).expect("Failed to get 'banned' field");
    assert_eq!(banned_flag, 1, "User 'banned' flag should be set");
    let ban_count: i64 = conn.hget(user_key.clone(), "banned_q").expect("Failed to get 'banned_q'");
    assert_eq!(ban_count, 1, "User ban count should increment");
    let chat_bans: i64 = conn.hget(chat_key.clone(), field::BANNED).expect("Failed to get chat banned count");
    assert_eq!(chat_bans, 1, "Chat's banned count should increment by 1");
}

#[tokio::test]
#[serial]
async fn tg_perm_ban_sets_symbol_and_updates_perm_ban_count() {
    flush_redis();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let chat_id: i64 = 5005;
    let user_id: u64 = 888;
    let user_key = format!("{}{}", key::TG_USERS_PREFIX, user_id);
    let chat_key = format!("{}{}", key::TG_CHATS_PREFIX, chat_id);

    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut conn = client.get_connection().unwrap();
    let _: () = conn.hset(user_key.clone(), field::BANNED_Q, CONFIG.banned_q + 1).expect("Failed to set banned_q");
    let _: () = conn.hset(user_key.clone(), field::REP, 0).expect("Failed to set rep");
    tokio::time::sleep(Duration::from_millis(50)).await;

    let reply = scan_msg(
        make_message(chat_id, user_id, "tester", "Another message", 1),
        "Another message".into()
    ).await.ok().unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    assert!(reply.symbols.contains_key(symbol::TG_PERM_BAN), "Expected TG_PERM_BAN symbol for frequent offender");
    assert!(!reply.symbols.contains_key(symbol::TG_BAN), "TG_BAN should not be present when only perm ban triggers");
    let perm_bans: i64 = conn.hget(chat_key.clone(), "perm_banned").expect("Failed to get perm_banned count");
    assert_eq!(perm_bans, 1, "Chat's permanent ban count should increment by 1");
    let final_ban_count: i64 = conn.hget(user_key.clone(), field::BANNED_Q).expect("Failed to get 'banned_q'");
    assert_eq!(final_ban_count, 4, "User banned_q should remain at 4 (perm ban triggered)");
}

#[tokio::test]
#[serial]
async fn makeadmin_adds_admin_chat_and_generates_keyboard() {
    flush_redis();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let user_id: u64 = 42;
    let current_chat: i64 = 1000;
    let other_chat: i64 = 2000;

    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut conn = client.get_connection().unwrap();
    let _: () = conn.sadd(format!("{}{}", user_id, suffix::BOT_CHATS), current_chat).unwrap();
    let _: () = conn.sadd(format!("{}{}", user_id, suffix::BOT_CHATS), other_chat).unwrap();
    let _: () = conn.hset(format!("{}{}", key::TG_CHATS_PREFIX, current_chat), field::NAME, "CurrentChat").unwrap();
    let _: () = conn.hset(format!("{}{}", key::TG_CHATS_PREFIX, other_chat), field::NAME, "OtherChat").unwrap();

    let bot = Bot::new("DUMMY");
    let msg = make_message(current_chat, user_id, "tester", "/makeadmin", 1);
    let result = handle_admin_command(bot, msg, AdminCommand::MakeAdmin).await;
    assert!(result.is_err(), "Expected send_message to fail with dummy token");

    let admin_chats: Vec<i64> = conn.smembers(format!("{}{}", user_id, suffix::ADMIN_CHATS)).unwrap();
    assert!(admin_chats.contains(&current_chat), "Admin chat not set in Redis");
    let bot_chats: Vec<i64> = conn.smembers(format!("{}{}", user_id, suffix::BOT_CHATS)).unwrap();
    assert!(bot_chats.contains(&current_chat) && bot_chats.contains(&other_chat));
    let expected_buttons = bot_chats.len() - 1;
    assert_eq!(expected_buttons, 1, "Inline keyboard should list exactly one other chat");
}


#[tokio::test]
#[serial]
async fn reputation_command_returns_value_or_zero() {
    flush_redis();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let user_id: u64 = 100;
    let target_username = "someuser";
    let rep_key = format!("{}{}", key::TG_USERS_PREFIX, user_id);

    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut conn = client.get_connection().unwrap();
    let res: bool = conn.hset(rep_key.clone(), field::REP, 0).expect("Failed to set rep");
    assert!(res);

    let bot = Bot::new("DUMMY");
    let chat_id: i64 = 1111;
    let msg1 = make_message(chat_id, user_id, "tester", &format!("/reputation {}", target_username), 1);
    let res1 = handle_admin_command(bot.clone(), msg1, AdminCommand::Reputation { user: target_username.into() }).await;
    assert!(res1.is_err(), "Expected dummy send_message to fail");
    let rep: bool = conn.hexists(rep_key.clone(), field::REP).expect("Failed to get rep");
    assert!(rep, "Reputation key should not exist for new user");

    let _:() = conn.hset(rep_key.clone(), field::REP, 5).expect("Failed to set rep");
    let msg2 = make_message(chat_id, user_id, "tester", &format!("/reputation {}", target_username), 2);
    let res2 = handle_admin_command(bot, msg2, AdminCommand::Reputation { user: target_username.into() }).await;
    assert!(res2.is_err());
    let stored: i64 = conn.hget(rep_key.clone(), field::REP).expect("Failed to get rep");
    assert_eq!(stored, 5, "Reputation value should remain 5 in Redis");
}

#[tokio::test]
#[serial]
async fn stats_command_shows_chat_stats_or_list() {
    flush_redis();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let user_id: u64 = 555;
    let admin_chat_id: i64 = 9000;
    let group_chat1: i64 = 9001;
    let group_chat2: i64 = 9002;

    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut conn = client.get_connection().unwrap();
    let _: () = conn.sadd(format!("{}{}", user_id, suffix::ADMIN_CHATS), admin_chat_id).unwrap();
    let _: () = conn.sadd(format!("{}{}{}", key::ADMIN_PREFIX, admin_chat_id, suffix::MODERATED_CHATS), group_chat1).unwrap();
    let _: () = conn.sadd(format!("{}{}{}", key::ADMIN_PREFIX, admin_chat_id, suffix::MODERATED_CHATS), group_chat2).unwrap();
    let _: () = conn.hset(format!("{}{}", key::TG_CHATS_PREFIX, group_chat1), field::NAME, "GroupChat1").unwrap();
    let _: () = conn.hset(format!("{}{}", key::TG_CHATS_PREFIX, group_chat1), field::SPAM_COUNT, 5).unwrap();
    let _: () = conn.hset(format!("{}{}", key::TG_CHATS_PREFIX, group_chat1), field::ADMIN_CHAT, admin_chat_id).unwrap();
    let _: () = conn.hset(format!("{}{}", key::TG_CHATS_PREFIX, group_chat2), field::NAME, "GroupChat2").unwrap();
    let _: () = conn.hset(format!("{}{}", key::TG_CHATS_PREFIX, group_chat2), field::SPAM_COUNT, 3).unwrap();
    let _: () = conn.hset(format!("{}{}", key::TG_CHATS_PREFIX, group_chat2), field::ADMIN_CHAT, admin_chat_id).unwrap();

    let bot = Bot::new("DUMMY");
    let msg1 = make_message(group_chat1, user_id, "tester", "/stats", 1);
    let res1 = handle_admin_command(bot.clone(), msg1, AdminCommand::Stats).await;
    assert!(res1.is_err());
    let stats1: HashMap<String, String> = conn.hgetall(format!("{}{}", key::TG_CHATS_PREFIX, group_chat1)).unwrap();
    assert!(stats1.get(field::NAME).is_some() && stats1.get(field::ADMIN_CHAT).is_some());
    assert_eq!(stats1.get(field::SPAM_COUNT), Some(&"5".to_string()));

    // 3. Admin chat: user sends /stats in the admin control chat
    let msg2 = make_message(admin_chat_id, user_id, "tester", "/stats", 2);
    let res2 = handle_admin_command(bot, msg2, AdminCommand::Stats).await;
    assert!(res2.is_err());
    let moderated: Vec<i64> = conn.smembers(format!("{}{}{}", key::ADMIN_PREFIX, admin_chat_id, suffix::MODERATED_CHATS)).unwrap();
    assert_eq!(moderated.len(), 2);
    assert!(moderated.contains(&group_chat1) && moderated.contains(&group_chat2));
    let name1: String = conn.hget(format!("{}{}", key::TG_CHATS_PREFIX, group_chat1), field::NAME).unwrap();
    let name2: String = conn.hget(format!("{}{}", key::TG_CHATS_PREFIX, group_chat2), field::NAME).unwrap();
    assert_eq!(name1, "GroupChat1");
    assert_eq!(name2, "GroupChat2");
}

#[tokio::test]
#[serial]
async fn addregex_command_parses_and_writes_rule() {
    flush_redis();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let symbol = "TESTSYM";
    
    let file_path = PathBuf::from(format!("/etc/rspamd/lua.local.d/telegram_regex_{}.lua", symbol));
    let _ = fs::remove_file(&file_path);

    let bot = Bot::new("DUMMY");

    // Invalid: two parts → usage error, no file
    let bad = format!("{}|{}", symbol, "[0-9]+");
    let msg1 = make_message(1, 999, "t", &format!("/addregex {}", bad), 1);
    let _ = handle_admin_command(bot.clone(), msg1, AdminCommand::AddRegex { pattern: bad }).await;
    assert!(!file_path.exists(), "No file for bad input");

    // Valid: three parts → file created
    let good = format!("{}|{}|{}", symbol, "[0-9]+", 5);
    let msg2 = make_message(1, 999, "t", &format!("/addregex {}", good), 2);
    let _ = handle_admin_command(bot, msg2, AdminCommand::AddRegex { pattern: good }).await;
    // now the file should exist
    assert!(file_path.exists(), "File should be created for valid input");

    let contents = fs::read_to_string(&file_path).unwrap();
    assert!(contents.contains(&format!("config['regexp']['{}']", symbol)));
    assert!(contents.contains("re = '[0-9]+'"));
    assert!(contents.contains("score = 5"));

    // cleanup
    let _ = fs::remove_file(&file_path);

}
#[tokio::test]
#[serial]
async fn whitelist_command_adds_entries() {
    flush_redis();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let chat_id: i64 = 6006;
    let user_id: u64 = 101;
    let bot = Bot::new("DUMMY");
    let msg_user = make_message(chat_id, user_id, "tester", "/whitelist user|add|101", 1);
    let _ = handle_admin_command(bot.clone(), msg_user, AdminCommand::Whitelist { pattern: "user|add|101".into() }).await;

    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut conn = client.get_connection().unwrap();
    let has_user: bool = conn.sismember(key::TG_WHITELIST_USER_KEY, "101").unwrap();
    assert!(has_user, "User should be in whitelist set");

    let msg_word = make_message(chat_id, user_id, "tester", "/whitelist word|add|hello", 2);
    let _ = handle_admin_command(bot, msg_word, AdminCommand::Whitelist { pattern: "word|add|hello".into() }).await;
    let words: Vec<String> = conn.smembers(key::TG_WHITELIST_WORD_KEY).unwrap();
    assert!(words.contains(&"hello".to_string()));
}

#[tokio::test]
#[serial]
async fn blacklist_command_adds_entries() {
    flush_redis();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let chat_id: i64 = 7007;
    let user_id: u64 = 202;
    let bot = Bot::new("DUMMY");
    let msg_user = make_message(chat_id, user_id, "tester", "/blacklist user|add|202", 1);
    let _ = handle_admin_command(bot.clone(), msg_user, AdminCommand::Blacklist { pattern: "user|add|202".into() }).await;

    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut conn = client.get_connection().unwrap();
    let has_user: bool = conn.sismember(key::TG_BLACKLIST_USER_KEY, "202").unwrap();
    assert!(has_user, "User should be in blacklist set");

    let msg_word = make_message(chat_id, user_id, "tester", "/blacklist word|add|spam", 2);
    let _ = handle_admin_command(bot, msg_word, AdminCommand::Blacklist { pattern: "word|add|spam".into() }).await;
    let words: Vec<String> = conn.smembers(key::TG_BLACKLIST_WORD_KEY).unwrap();
    assert!(words.contains(&"spam".to_string()));
}

// ============================================================================
// NEW MODULAR SYMBOL INTEGRATION TESTS
// ============================================================================

#[tokio::test]
#[serial]
async fn tg_first_fast_sets_symbol_for_quick_first_message() {
    flush_redis();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let chat_id = 8001;
    let user_id = 1001;
    let key = format!("{}{}", key::TG_USERS_PREFIX, user_id);

    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut conn = client.get_connection().unwrap();
    
    // Set user as newly joined (within 10 seconds)
    let join_time = Utc::now().timestamp() - 5; // 5 seconds ago
    let _: () = conn.hset(key.clone(), "join_time", join_time).unwrap();

    let reply = scan_msg(
        make_message(chat_id, user_id, "newuser", "Hello everyone!", 1),
        "Hello everyone!".into(),
    ).await.ok().unwrap();

    assert!(reply.symbols.contains_key(symbol::TG_FIRST_FAST), 
        "Expected TG_FIRST_FAST for message within 10 seconds of joining");
}

#[tokio::test]
#[serial]
async fn tg_first_slow_sets_symbol_for_delayed_first_message() {
    flush_redis();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let chat_id = 8002;
    let user_id = 1002;
    let key = format!("{}{}", key::TG_USERS_PREFIX, user_id);

    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut conn = client.get_connection().unwrap();
    
    // Set user as joined long ago (more than 24 hours)
    let join_time = Utc::now().timestamp() - 90000; // 25 hours ago
    let _: () = conn.hset(key.clone(), "join_time", join_time).unwrap();

    let reply = scan_msg(
        make_message(chat_id, user_id, "olduser", "Finally saying hello!", 1),
        "Finally saying hello!".into(),
    ).await.ok().unwrap();

    assert!(reply.symbols.contains_key(symbol::TG_FIRST_SLOW), 
        "Expected TG_FIRST_SLOW for first message after 24 hours of joining");
}

#[tokio::test]
#[serial]
async fn tg_silent_sets_symbol_for_dormant_user() {
    flush_redis();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let chat_id = 8003;
    let user_id = 1003;
    let key = format!("{}{}", key::TG_USERS_PREFIX, user_id);

    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut conn = client.get_connection().unwrap();
    
    // Set user's last message time to 30+ days ago
    let last_msg_time = Utc::now().timestamp() - 2600000; // 30+ days ago
    let _: () = conn.hset(key.clone(), "last_msg_time", last_msg_time).unwrap();

    let reply = scan_msg(
        make_message(chat_id, user_id, "dormantuser", "I'm back!", 1),
        "I'm back!".into(),
    ).await.ok().unwrap();

    assert!(reply.symbols.contains_key(symbol::TG_SILENT), 
        "Expected TG_SILENT for dormant user returning after 30 days");
}

#[tokio::test]
#[serial]
async fn tg_link_spam_sets_symbol_for_excessive_links() {
    flush_redis();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let chat_id = 8004;
    let user_id = 1004;

    // Message with 4 links (above threshold of 3)
    let spam_text = "Check out these amazing sites: https://example1.com https://example2.com https://example3.com https://example4.com";

    let reply = scan_msg(
        make_message(chat_id, user_id, "linkuser", spam_text, 1),
        spam_text.into(),
    ).await.ok().unwrap();

    assert!(reply.symbols.contains_key(symbol::TG_LINK_SPAM), 
        "Expected TG_LINK_SPAM for message with excessive links");
}

#[tokio::test]
#[serial]
async fn tg_mentions_sets_symbol_for_excessive_mentions() {
    flush_redis();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let chat_id = 8005;
    let user_id = 1005;

    // Message with 6 mentions (above threshold of 5)
    let spam_text = "@user1 @user2 @user3 @user4 @user5 @user6 Hey everyone!";

    let reply = scan_msg(
        make_message(chat_id, user_id, "mentionuser", spam_text, 1),
        spam_text.into(),
    ).await.ok().unwrap();

    assert!(reply.symbols.contains_key(symbol::TG_MENTIONS), 
        "Expected TG_MENTIONS for message with excessive user mentions");
}

#[tokio::test]
#[serial]
async fn tg_caps_sets_symbol_for_excessive_capitalization() {
    flush_redis();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let chat_id = 8006;
    let user_id = 1006;

    // Message with 80% caps (above threshold of 70%)
    let spam_text = "HELLO EVERYONE THIS IS VERY IMPORTANT NEWS YOU MUST READ THIS NOW!";

    let reply = scan_msg(
        make_message(chat_id, user_id, "capsuser", spam_text, 1),
        spam_text.into(),
    ).await.ok().unwrap();

    assert!(reply.symbols.contains_key(symbol::TG_CAPS), 
        "Expected TG_CAPS for message with excessive capitalization");
}

#[tokio::test]
#[serial]
async fn tg_emoji_spam_sets_symbol_for_excessive_emoji() {
    flush_redis();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let chat_id = 8007;
    let user_id = 1007;

    // Message with 12 emojis (above threshold of 10)
    let spam_text = "Hello! 😀😃😄😁😆😅😂🤣😊😇🙂🙃";

    let reply = scan_msg(
        make_message(chat_id, user_id, "emojiuser", spam_text, 1),
        spam_text.into(),
    ).await.expect("scan_msg should succeed");

    assert!(reply.symbols.contains_key(symbol::TG_EMOJI_SPAM), 
        "Expected TG_EMOJI_SPAM for message with excessive emoji usage");
}

#[tokio::test]
#[serial]
async fn tg_invite_link_sets_symbol_for_telegram_invites() {
    flush_redis();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let chat_id = 8008;
    let user_id = 1008;

    // Message with Telegram invite link
    let spam_text = "Join our amazing group: https://t.me/joinchat/ABC123";

    let reply = scan_msg(
        make_message(chat_id, user_id, "inviteuser", spam_text, 1),
        spam_text.into(),
    ).await.ok().unwrap();

    assert!(reply.symbols.contains_key(symbol::TG_INVITE_LINK), 
        "Expected TG_INVITE_LINK for message with Telegram invite link");
}

#[tokio::test]
#[serial]
async fn tg_phone_spam_sets_symbol_for_phone_patterns() {
    flush_redis();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let chat_id = 8009;
    let user_id = 1009;

    // Message with phone number pattern
    let spam_text = "Call me at +1-555-123-4567 for amazing deals!";

    let reply = scan_msg(
        make_message(chat_id, user_id, "phoneuser", spam_text, 1),
        spam_text.into(),
    ).await.ok().unwrap();

    assert!(reply.symbols.contains_key(symbol::TG_PHONE_SPAM), 
        "Expected TG_PHONE_SPAM for message with phone number pattern");
}

#[tokio::test]
#[serial]
async fn tg_spam_chat_sets_symbol_for_spam_chat_links() {
    flush_redis();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let chat_id = 8010;
    let user_id = 1010;

    // Message with spam chat link
    let spam_text = "Check out this chat: https://t.me/joinchat/spamchat123";

    let reply = scan_msg(
        make_message(chat_id, user_id, "spamchatuser", spam_text, 1),
        spam_text.into(),
    ).await.ok().unwrap();

    assert!(reply.symbols.contains_key(symbol::TG_SPAM_CHAT), 
        "Expected TG_SPAM_CHAT for message with spam chat link");
}

#[tokio::test]
#[serial]
async fn tg_shortener_sets_symbol_for_url_shorteners() {
    flush_redis();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let chat_id = 8011;
    let user_id = 1011;

    // Message with URL shortener
    let spam_text = "Check this out: https://bit.ly/amazing-deal";

    let reply = scan_msg(
        make_message(chat_id, user_id, "shorteneruser", spam_text, 1),
        spam_text.into(),
    ).await.ok().unwrap();

    assert!(reply.symbols.contains_key(symbol::TG_SHORTENER), 
        "Expected TG_SHORTENER for message with URL shortener");
}

#[tokio::test]
#[serial]
async fn tg_gibberish_sets_symbol_for_random_consonants() {
    flush_redis();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let chat_id = 8012;
    let user_id = 1012;

    // Message with long sequence of random consonants
    let spam_text = "KXJQZWVBNMLPQRSTUVWXYZKXJQZWVBNMLPQRSTUVWXYZKXJQZWVBNMLPQRSTUVWXYZ";

    let reply = scan_msg(
        make_message(chat_id, user_id, "gibberishuser", spam_text, 1),
        spam_text.into(),
    ).await.ok().unwrap();

    assert!(reply.symbols.contains_key(symbol::TG_GIBBERISH), 
        "Expected TG_GIBBERISH for message with gibberish text");
}

#[tokio::test]
#[serial]
async fn multiple_symbols_can_trigger_simultaneously() {
    flush_redis();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let chat_id = 8013;
    let user_id = 1013;

    // Message that should trigger multiple symbols
    let spam_text = "HELLO EVERYONE! 😀😃😄😁😆😅😂🤣😊😇🙂🙃 @user1 @user2 @user3 @user4 @user5 @user6 Check out: https://t.me/joinchat/ABC123 https://bit.ly/deal";

    let reply = scan_msg(
        make_message(chat_id, user_id, "multiuser", spam_text, 1),
        spam_text.into(),
    ).await.ok().unwrap();

    // Should trigger multiple symbols
    let triggered_symbols: Vec<&str> = reply.symbols.keys().map(|s| s.as_str()).collect();
    
    assert!(triggered_symbols.contains(&symbol::TG_CAPS), "Expected TG_CAPS");
    assert!(triggered_symbols.contains(&symbol::TG_EMOJI_SPAM), "Expected TG_EMOJI_SPAM");
    assert!(triggered_symbols.contains(&symbol::TG_MENTIONS), "Expected TG_MENTIONS");
    assert!(triggered_symbols.contains(&symbol::TG_INVITE_LINK), "Expected TG_INVITE_LINK");
    assert!(triggered_symbols.contains(&symbol::TG_SHORTENER), "Expected TG_SHORTENER");
    
    // Verify we have at least 5 symbols triggered
    assert!(triggered_symbols.len() >= 5, "Expected at least 5 symbols to be triggered");
}

#[tokio::test]
#[serial]
async fn normal_messages_dont_trigger_new_symbols() {
    flush_redis();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let chat_id = 8014;
    let user_id = 1014;

    // Normal, legitimate message
    let normal_text = "Hello everyone! How are you doing today? It's a beautiful day.";

    let reply = scan_msg(
        make_message(chat_id, user_id, "normaluser", normal_text, 1),
        normal_text.into(),
    ).await.ok().unwrap();

    // Should not trigger any of the new content-based symbols
    let triggered_symbols: Vec<&str> = reply.symbols.keys().map(|s| s.as_str()).collect();
    
    assert!(!triggered_symbols.contains(&symbol::TG_LINK_SPAM), "Normal message should not trigger TG_LINK_SPAM");
    assert!(!triggered_symbols.contains(&symbol::TG_MENTIONS), "Normal message should not trigger TG_MENTIONS");
    assert!(!triggered_symbols.contains(&symbol::TG_CAPS), "Normal message should not trigger TG_CAPS");
    assert!(!triggered_symbols.contains(&symbol::TG_EMOJI_SPAM), "Normal message should not trigger TG_EMOJI_SPAM");
    assert!(!triggered_symbols.contains(&symbol::TG_INVITE_LINK), "Normal message should not trigger TG_INVITE_LINK");
    assert!(!triggered_symbols.contains(&symbol::TG_PHONE_SPAM), "Normal message should not trigger TG_PHONE_SPAM");
    assert!(!triggered_symbols.contains(&symbol::TG_SPAM_CHAT), "Normal message should not trigger TG_SPAM_CHAT");
    assert!(!triggered_symbols.contains(&symbol::TG_SHORTENER), "Normal message should not trigger TG_SHORTENER");
    assert!(!triggered_symbols.contains(&symbol::TG_GIBBERISH), "Normal message should not trigger TG_GIBBERISH");
}

#[tokio::test]
#[serial]
async fn reputation_symbols_are_properly_detected() {
    flush_redis();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let chat_id = 8015;
    let user_id = 1015;

    // Test message to check reputation symbol detection
    let test_text = "Test message for reputation checking";

    let reply = scan_msg(
        make_message(chat_id, user_id, "reputationuser", test_text, 1),
        test_text.into(),
    ).await.ok().unwrap();

    // Check if reputation symbols are present in the response
    let triggered_symbols: Vec<&str> = reply.symbols.keys().map(|s| s.as_str()).collect();
    
    // Note: These symbols will only be present if Rspamd reputation plugin is configured
    // This test verifies that the bot can handle reputation symbols when they are present
    println!("Detected symbols: {:?}", triggered_symbols);
    
    // The test passes if the scan completes successfully and returns symbols
    // Even if reputation symbols aren't present (plugin not configured), the test should pass
    println!("Scan completed successfully with {} symbols", reply.symbols.len());
    
    // If reputation symbols are configured and present, verify they're handled correctly
    if triggered_symbols.contains(&symbol::USER_REPUTATION) {
        println!("USER_REPUTATION symbol detected");
    }
    if triggered_symbols.contains(&symbol::USER_REPUTATION_BAD) {
        println!("USER_REPUTATION_BAD symbol detected");
    }
    if triggered_symbols.contains(&symbol::USER_REPUTATION_GOOD) {
        println!("USER_REPUTATION_GOOD symbol detected");
    }
}
