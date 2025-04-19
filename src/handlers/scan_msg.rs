use chrono::{DateTime, Local, LocalResult, TimeZone, Utc};
use rspamd_client::{config::Config, error::RspamdError, protocol::RspamdScanReply, scan_async};
use teloxide::prelude::*;

pub async fn scan_msg(msg: Message, text: String) -> Result<RspamdScanReply, RspamdError> {
    let user = msg.from.unwrap();
    let user_id = user.id.to_string();
    let chat_id = msg.chat.id;
    let date = eml_date_from_timestamp(msg.date.timestamp());
    let text = text;
    let email = format!(
        "Date: {date}\r\n\
        From: telegram{user}@local\r\n\
        To: telegram{chat}@local\r\n\
        Subject: Telegram message\r\n\
        X-Telegram-User: {user}\r\n\
        \r\n\
        {text}\r\n",
        date = date,
        user = user_id,
        chat = chat_id,
        text = text.replace("\n", "\r\n") // если в самом тексте тоже могут быть переводы строк
    );
    let options = Config::builder()
        .base_url("http://localhost:11333".to_string())
        .build();
    scan_async(&options, email).await
}

fn eml_date_from_timestamp(ts: i64) -> String {
    // 1. Safely construct a DateTime<Utc> via timestamp_opt
    let dt_utc: DateTime<Utc> = match Utc.timestamp_opt(ts, 0) {
        LocalResult::Single(dt) => dt,
        LocalResult::Ambiguous(dt1, _) => dt1, // unlikely for epoch seconds
        LocalResult::None => Utc::now(),        // fallback if out of range
    };

    // 2. Convert into the local timezone
    let dt_local: DateTime<Local> = dt_utc.with_timezone(&Local);

    // 3. Format per RFC 5322: "Day, DD Mon YYYY HH:MM:SS ±ZZZZ"
    dt_local.format("%a, %d %b %Y %H:%M:%S %z").to_string()
}
