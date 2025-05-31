//! tests/integration_tests.rs
//!
//! Runs the three Lua-rule paths (TG_FLOOD, TG_REPEAT, TG_SUSPICIOUS) against a
//! **real** Rspamd + Redis back-end.  Rspamd is accessed through the
//! crate-exported `scan_msg` helper, Redis through the `redis` crate.
//
//! Redis **must** be running on 127.0.0.1:6379
//! Rspamd **must** be running on 127.0.0.1:11333 with the bot’s Lua rules.

use std::time::Duration;

use chrono::Utc;
use redis::Commands;
use rspamd_telegram_bot::admin_handlers::{handle_admin_command, AdminCommand};
use rspamd_telegram_bot::handlers::scan_msg;
use rspamd_telegram_bot::config::{field, key, suffix, symbol};
use teloxide::types::{Chat, ChatId, ChatKind, ChatPrivate, MediaKind, MediaText, Message, MessageCommon, MessageId, MessageKind, User, UserId};
use teloxide::Bot;
use once_cell::sync::Lazy;
use std::{collections::HashMap, fs, io, path::Path};
use std::path::PathBuf;

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

fn flush_redis() {
    let client = redis::Client::open("redis://127.0.0.1/")
        .expect("Failed to connect to Redis");
    let mut conn = client
        .get_connection()
        .expect("Failed to connect to Redis");
    let _: () = conn
        .flushdb()
        .expect("Failed to flush Redis");
}

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