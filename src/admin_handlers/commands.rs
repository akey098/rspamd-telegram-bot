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
}