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
    /// Prefix for trusted message IDs (e.g. `"tg:trusted:<message_id>"`)
    pub const TG_TRUSTED_PREFIX: &str = "tg:trusted:";
    /// Prefix for reply tracking (e.g. `"tg:replies:<chat_id>:<message_id>"`)
    pub const TG_REPLIES_PREFIX: &str = "tg:replies:";
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
    /// Suffix for trusted message metadata (e.g. `"<message_id>:metadata"`)
    pub const TRUSTED_METADATA: &str = ":metadata";
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
    /// Field storing trusted message sender ID
    pub const TRUSTED_SENDER: &str = "trusted_sender";
    /// Field storing trusted message chat ID
    pub const TRUSTED_CHAT: &str = "trusted_chat";
    /// Field storing trusted message timestamp
    pub const TRUSTED_TIMESTAMP: &str = "trusted_timestamp";
    /// Field storing trusted message type (bot, admin, verified)
    pub const TRUSTED_TYPE: &str = "trusted_type";
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
    
    // Reputation-based symbols (from Rspamd reputation plugin)
    /// Symbol for user reputation (from Rspamd reputation plugin)
    pub const USER_REPUTATION: &str = "USER_REPUTATION";
    /// Symbol for bad user reputation
    pub const USER_REPUTATION_BAD: &str = "USER_REPUTATION_BAD";
    /// Symbol for good user reputation
    pub const USER_REPUTATION_GOOD: &str = "USER_REPUTATION_GOOD";
    
    // Reply-aware filtering symbols
    /// Symbol for reply to trusted message (`TG_REPLY`)
    pub const TG_REPLY: &str = "TG_REPLY";
    /// Symbol for reply to bot message (`TG_REPLY_BOT`)
    pub const TG_REPLY_BOT: &str = "TG_REPLY_BOT";
    /// Symbol for reply to admin message (`TG_REPLY_ADMIN`)
    pub const TG_REPLY_ADMIN: &str = "TG_REPLY_ADMIN";
    /// Symbol for reply to verified user message (`TG_REPLY_VERIFIED`)
    pub const TG_REPLY_VERIFIED: &str = "TG_REPLY_VERIFIED";
}

/// Features that are enabled for every chat by default.
/// This now includes all available features to provide comprehensive spam protection by default.
pub const DEFAULT_FEATURES: &[&str] = &[
    // Core features (from core.lua)
    "flood",
    "repeat", 
    "suspicious",
    "ban",
    "perm_ban",
    
    // Content features (from content.lua)
    "link_spam",
    "mentions",
    "caps",
    "emoji_spam",
    
    // Timing features (from timing.lua)
    "first_fast",
    "first_slow", 
    "silent",
    
    // List features (from lists.lua)
    "whitelist",
    "blacklist",
    
    // Heuristic features (from heuristics.lua)
    "invite_link",
    "phone_spam",
    "spam_chat",
    "shortener",
    "gibberish",
    
    // Reply-aware filtering features
    "reply_aware",
    "trusted_replies",
];

/// Redis key storing the global set of features enabled by default.
pub const ENABLED_FEATURES_KEY: &str = "tg:enabled_features";

/// Ban counter reduction interval in seconds (48 hours)
pub const BAN_COUNTER_REDUCTION_INTERVAL: i64 = 48 * 60 * 60; // 48 hours in seconds

/// Trusted message TTL in seconds (24 hours)
pub const TRUSTED_MESSAGE_TTL: i64 = 24 * 60 * 60; // 24 hours in seconds

/// Reply tracking TTL in seconds (7 days)
pub const REPLY_TRACKING_TTL: i64 = 7 * 24 * 60 * 60; // 7 days in seconds

/// Advanced Reply-Aware Filtering Configuration
pub mod reply_aware {
    /// Maximum number of trusted messages a user can create per hour
    pub const MAX_TRUSTED_MESSAGES_PER_HOUR: u32 = 10;
    
    /// Maximum number of replies to trusted messages per hour
    pub const MAX_REPLIES_PER_HOUR: u32 = 50;
    
    /// Minimum time between trusted message creation (seconds)
    pub const MIN_TRUSTED_MESSAGE_INTERVAL: u64 = 60; // 1 minute
    
    /// Rate limiting window for trusted message creation (seconds)
    pub const TRUSTED_MESSAGE_RATE_WINDOW: u64 = 3600; // 1 hour
    
    /// Rate limiting window for replies (seconds)
    pub const REPLY_RATE_WINDOW: u64 = 3600; // 1 hour
    
    /// Maximum score reduction for replies (prevents abuse)
    pub const MAX_SCORE_REDUCTION: f64 = -5.0;
    
    /// Minimum score for spam patterns in replies (even to trusted messages)
    pub const MIN_SPAM_SCORE_IN_REPLIES: f64 = 1.0;
    
    /// Enable selective trusting (only trust specific message types)
    pub const ENABLE_SELECTIVE_TRUSTING: bool = true;
    
    /// Enable anti-evasion measures
    pub const ENABLE_ANTI_EVASION: bool = true;
    
    /// Enable rate limiting for trusted message creation
    pub const ENABLE_RATE_LIMITING: bool = true;
    
    /// Enable monitoring for spam patterns in replies
    pub const ENABLE_SPAM_MONITORING: bool = true;
    
    /// Trust levels configuration
    pub mod trust_levels {
        /// Trust level for bot messages (highest)
        pub const BOT_TRUST_LEVEL: f64 = -3.0;
        
        /// Trust level for admin messages (medium)
        pub const ADMIN_TRUST_LEVEL: f64 = -2.0;
        
        /// Trust level for verified user messages (low)
        pub const VERIFIED_TRUST_LEVEL: f64 = -1.0;
        
        /// Trust level for regular user messages (none)
        pub const REGULAR_TRUST_LEVEL: f64 = 0.0;
    }
    
    /// Anti-evasion thresholds
    pub mod anti_evasion {
        /// Maximum links allowed in reply to trusted message
        pub const MAX_LINKS_IN_REPLY: u32 = 2;
        
        /// Maximum phone numbers allowed in reply to trusted message
        pub const MAX_PHONE_NUMBERS_IN_REPLY: u32 = 1;
        
        /// Maximum invite links allowed in reply to trusted message
        pub const MAX_INVITE_LINKS_IN_REPLY: u32 = 0;
        
        /// Maximum caps ratio allowed in reply to trusted message
        pub const MAX_CAPS_RATIO_IN_REPLY: f64 = 0.5;
        
        /// Maximum emoji count allowed in reply to trusted message
        pub const MAX_EMOJI_IN_REPLY: u32 = 5;
    }
}

/// Redis keys for rate limiting and anti-evasion
pub mod rate_limit {
    /// Prefix for trusted message rate limiting (e.g. `"tg:rate:trusted:<user_id>"`)
    pub const TRUSTED_MESSAGE_RATE_PREFIX: &str = "tg:rate:trusted:";
    
    /// Prefix for reply rate limiting (e.g. `"tg:rate:replies:<user_id>"`)
    pub const REPLY_RATE_PREFIX: &str = "tg:rate:replies:";
    
    /// Prefix for spam pattern monitoring (e.g. `"tg:spam:replies:<user_id>"`)
    pub const SPAM_PATTERN_PREFIX: &str = "tg:spam:replies:";
}

/// Configuration for selective trusting
pub mod selective_trust {
    /// Only trust replies to bot messages
    pub const TRUST_BOT_MESSAGES: bool = true;
    
    /// Only trust replies to admin messages
    pub const TRUST_ADMIN_MESSAGES: bool = true;
    
    /// Only trust replies to verified user messages
    pub const TRUST_VERIFIED_MESSAGES: bool = false;
    
    /// Only trust replies to messages from users with good reputation
    pub const TRUST_GOOD_REPUTATION: bool = true;
    
    /// Minimum reputation score required for trusting
    pub const MIN_REPUTATION_FOR_TRUST: i64 = -2;
    
    /// Only trust replies to recent messages (within last hour)
    pub const TRUST_RECENT_MESSAGES_ONLY: bool = true;
    
    /// Maximum age of trusted message (seconds)
    pub const MAX_TRUSTED_MESSAGE_AGE: u64 = 3600; // 1 hour
}
