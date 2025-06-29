//! Centralized configuration for Redis keys, fields, and other static constants.

/// **Redis Key Prefixes:** identify categories of data stored in Redis.
pub mod key {
    /// Prefix for user data keys in Redis (e.g. `"tg:users:<user_id>"`).
    pub const TG_USERS_PREFIX: &str = "tg:users:";
    /// Prefix for chat data keys in Redis (e.g. `"tg:chats:<chat_id>"`).
    pub const TG_CHATS_PREFIX: &str = "tg:chats:";
    /// Prefix for admin chat keys (for storing moderated chats, e.g. `"admin:<admin_chat_id>:..."`).
    pub const ADMIN_PREFIX: &str = "admin:";
    /// Key for whitelist of users
    pub const TG_WHITELIST_USER_KEY: &str = "tg:whitelist:users";
    /// Key for blacklist of users
    pub const TG_BLACKLIST_USER_KEY: &str = "tg:blacklist:users";
    /// Key for whitelist of words
    pub const TG_WHITELIST_WORD_KEY: &str = "tg:whitelist:words";
    /// Key for blacklist of words
    pub const TG_BLACKLIST_WORD_KEY: &str = "tg:blacklist:words";
}

/// **Redis Key Suffixes:** common endings for composite Redis keys.
pub mod suffix {
    /// Suffix for a user's set of bot-accessible chats (e.g. `"<user_id>:bot_chats"`).
    pub const BOT_CHATS: &str = ":bot_chats";
    /// Suffix for a user's set of admin control chats (e.g. `"<user_id>:admin_chats"`).
    pub const ADMIN_CHATS: &str = ":admin_chats";
    /// Suffix for an admin chat's set of moderated chats (e.g. `"admin:<id>:moderated_chats"`).
    pub const MODERATED_CHATS: &str = ":moderated_chats";
    /// Suffix for admins of the chat
    pub const ADMINS: &str = ":admins";
}

/// **Redis Hash Field Names:** keys within Redis hashes for user/chat properties.
pub mod field {
    /// Field for storing a chat's name in the `tg:chats:<id>` hash.
    pub const NAME: &str = "name";
    /// Field for linking a chat to its admin control chat (stores admin chat ID).
    pub const ADMIN_CHAT: &str = "admin_chat";
    /// Field counting spam messages in a chat (spam score count).
    pub const SPAM_COUNT: &str = "spam_count";
    /// Field counting messages deleted by the bot in a chat.
    pub const DELETED: &str = "deleted";
    /// Field tracking a user's reputation score in their hash.
    pub const REP: &str = "rep";
    /// Field tracking recent message count for flood detection (in user hash).
    pub const FLOOD: &str = "flood";
    /// Field tracking consecutive equal messages count (for repeat detection).
    pub const EQ_MSG_COUNT: &str = "eq_msg_count";
    /// Field indicating a user has been banned (in user hash), or count of bans (in chat hash).
    pub const BANNED: &str = "banned";
    /// Field storing the last message content seen (for repeat detection logic).
    pub const LAST_MSG: &str = "last_msg";
    /// Field storing the username of the sender
    pub const USERNAME: &str = "username";
    /// Field storing the quantity of times user have been banned for the time
    pub const BANNED_Q: &str = "banned_q";
    /// Field storing the quantity of permanently banned users in the chat
    pub const PERM_BANNED: &str = "perm_banned";
}

/// **Rspamd Symbol Names:** spam detection symbols used by Rspamd and the bot.
pub mod symbol {
    /// Symbol for flooding behavior detected (`TG_FLOOD`).
    pub const TG_FLOOD: &str = "TG_FLOOD";
    /// Symbol for repeated message content detected (`TG_REPEAT`).
    pub const TG_REPEAT: &str = "TG_REPEAT";
    /// Symbol for suspicious user activity detected (`TG_SUSPICIOUS`).
    pub const TG_SUSPICIOUS: &str = "TG_SUSPICIOUS";
    /// Symbol for ban-worthy spam activity detected (`TG_BAN`).
    pub const TG_BAN: &str = "TG_BAN";
    /// Symbol for permanently banned user (`TG_PERM_BAN`)
    pub const TG_PERM_BAN: &str = "TG_PERM_BAN";
    
    // Timing-based symbols
    /// Symbol for first message too soon after joining (`TG_FIRST_FAST`).
    pub const TG_FIRST_FAST: &str = "TG_FIRST_FAST";
    /// Symbol for first message too long after joining (`TG_FIRST_SLOW`).
    pub const TG_FIRST_SLOW: &str = "TG_FIRST_SLOW";
    /// Symbol for dormant user returning after long silence (`TG_SILENT`).
    pub const TG_SILENT: &str = "TG_SILENT";
    
    // Content-based symbols
    /// Symbol for excessive links in message (`TG_LINK_SPAM`).
    pub const TG_LINK_SPAM: &str = "TG_LINK_SPAM";
    /// Symbol for excessive user mentions (`TG_MENTIONS`).
    pub const TG_MENTIONS: &str = "TG_MENTIONS";
    /// Symbol for excessive capital letters (`TG_CAPS`).
    pub const TG_CAPS: &str = "TG_CAPS";
    /// Symbol for excessive emoji usage (`TG_EMOJI_SPAM`).
    pub const TG_EMOJI_SPAM: &str = "TG_EMOJI_SPAM";
    
    // Heuristic-based symbols
    /// Symbol for Telegram invite links (`TG_INVITE_LINK`).
    pub const TG_INVITE_LINK: &str = "TG_INVITE_LINK";
    /// Symbol for phone number spam patterns (`TG_PHONE_SPAM`).
    pub const TG_PHONE_SPAM: &str = "TG_PHONE_SPAM";
    /// Symbol for spam chat links (`TG_SPAM_CHAT`).
    pub const TG_SPAM_CHAT: &str = "TG_SPAM_CHAT";
    /// Symbol for URL shortener links (`TG_SHORTENER`).
    pub const TG_SHORTENER: &str = "TG_SHORTENER";
    /// Symbol for gibberish text patterns (`TG_GIBBERISH`).
    pub const TG_GIBBERISH: &str = "TG_GIBBERISH";
    
    /// Symbol for whitelist of users
    pub const WHITELIST_USER: &str = "WHITELIST_USER";
    /// Symbol for blacklist of users
    pub const BLACKLIST_USER: &str = "BLACKLIST_USER";
    /// Symbol for whitelist of words
    pub const WHITELIST_WORD: &str = "WHITELIST_WORD";
    /// Symbol for blacklist of words
    pub const BLACKLIST_WORD: &str = "BLACKLIST_WORD";
}

/// Features that are enabled for every chat by default.
pub const DEFAULT_FEATURES: &[&str] = &[
    "flood",
    "repeat",
    "whitelist",
    "blacklist",
];

/// Redis key storing the global set of features enabled by default.
pub const ENABLED_FEATURES_KEY: &str = "tg:enabled_features";
