use teloxide::{utils::command::BotCommands};

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "Admin commands:")]
pub enum AdminCommand {
    #[command(description = "show help.")]
    Help,
    #[command(description = "show spam stats.")]
    Stats,
    #[command(description = "show user reputation.")]
    Reputation { user: String },
    #[command(description = "add a regex filter.")]
    AddRegex { pattern: String },
    #[command(description = "make this chat admin-chat.")]
    MakeAdmin,
    #[command(description = "show whitelist of users/words or add user/word to whitelist.")]
    Whitelist { pattern: String },
    #[command(description = "show blacklist of users/words or add user/word to blacklist.")]
    Blacklist { pattern: String },
    #[command(description = "Start managing features (callback flow)")]
    ManageFeatures,
    #[command(description = "mark a message as trusted for reply-aware filtering.")]
    MarkTrusted { args: String },
    #[command(description = "show trust management statistics.")]
    TrustStats,
    #[command(description = "configure reply-aware filtering settings.")]
    ReplyConfig { args: String },
    #[command(description = "show rate limiting statistics.")]
    RateLimitStats,
    #[command(description = "reset rate limiting for a user.")]
    ResetRateLimit { user: String },
    #[command(description = "show spam pattern history for a user.")]
    SpamPatterns { user: String },
    #[command(description = "configure selective trusting rules.")]
    SelectiveTrust { args: String },
    #[command(description = "show anti-evasion statistics.")]
    AntiEvasionStats,
    #[command(description = "learn a message as spam for Bayesian classifier.")]
    LearnSpam { message_id: String },
    #[command(description = "learn a message as ham for Bayesian classifier.")]
    LearnHam { message_id: String },
    #[command(description = "show Bayesian classifier statistics.")]
    BayesStats,
    #[command(description = "reset all Bayesian classifier data.")]
    BayesReset,
    #[command(description = "show neural network statistics.")]
    NeuralStats,
    #[command(description = "reset neural network model and training data.")]
    NeuralReset,
    #[command(description = "show neural network training status.")]
    NeuralStatus,
    #[command(description = "show neural network feature analysis.")]
    NeuralFeatures { message_id: String },
    #[command(description = "list recent messages stored in Redis (for debugging).")]
    ListMessages,
    #[command(description = "check learning status of a specific message.")]
    CheckMessage { message_id: String },
}