use teloxide::{utils::command::BotCommands};

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "Admin commands:")]
pub(crate) enum AdminCommand {
    #[command(description = "show help.")]
    Help,
    #[command(description = "enable a feature.")]
    Enable { feature: String },
    #[command(description = "disable a feature.")]
    Disable { feature: String },
    #[command(description = "show spam stats.")]
    Stats,
    #[command(description = "show user reputation.")]
    Reputation { user: String },
    #[command(description = "show recent flagged messages.")]
    Recent,
    #[command(description = "add a regex filter.")]
    AddRegex { pattern: String },
    #[command(description = "make this chat admin-chat.")]
    MakeAdmin,
}
