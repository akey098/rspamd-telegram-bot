//! Admin Panel Integration with Existing Bot System

use anyhow::Result;
use teloxide::{
    prelude::*,
    types::{Message, Update, BotCommandScope},
    utils::command::BotCommands,
    payloads::SetMyCommandsSetters,
};

use crate::admin_panel::commands::{AdminPanelCommand, handle_admin_panel_command};

/// Helper function to parse commands that may have bot username appended
fn parse_command_with_botname<T: BotCommands>(text: &str, bot_name: &str) -> Result<T, teloxide::utils::command::ParseError> {
    T::parse(text, bot_name).or_else(|_| {
        // If parsing fails, try to extract command without bot username
        if text.starts_with('/') {
            let parts: Vec<&str> = text.splitn(2, '@').collect();
            if parts.len() == 2 {
                // Command has @botname format, try parsing just the command part
                T::parse(parts[0], bot_name)
            } else {
                // No @ found, return original error
                T::parse(text, bot_name)
            }
        } else {
            // Not a command, return original error
            T::parse(text, bot_name)
        }
    })
}

/// Check if a message contains an admin panel command
pub fn is_admin_panel_command(text: &str) -> bool {
    parse_command_with_botname::<AdminPanelCommand>(text, "rspamd-bot").is_ok()
}

/// Handle admin panel commands in the message handler
pub async fn handle_admin_panel_message(bot: Bot, msg: Message) -> Result<()> {
    if let Some(text) = msg.text() {
        if let Ok(cmd) = parse_command_with_botname::<AdminPanelCommand>(text, "rspamd-bot") {
            handle_admin_panel_command(bot, msg, cmd).await?;
        }
    }
    Ok(())
}

/// Enhanced integration with existing message handler
/// This function shows how to integrate admin panel commands with the existing bot system
pub async fn integrated_message_handler(bot: Bot, msg: Message) -> Result<()> {
    if let Some(text) = msg.text() {
        // First check if it's an admin panel command
        if let Ok(cmd) = parse_command_with_botname::<AdminPanelCommand>(text, "rspamd-bot") {
            handle_admin_panel_command(bot.clone(), msg.clone(), cmd).await?;
            return Ok(());
        }
        
        // If not an admin panel command, handle with existing admin commands
        if let Ok(cmd) = parse_command_with_botname::<crate::admin_handlers::AdminCommand>(text, "rspamd-bot") {
            crate::admin_handlers::handle_admin_command(bot.clone(), msg.clone(), cmd).await?;
            return Ok(());
        }
        
        // If not an admin command, handle as regular message
        crate::handlers::handle_message(bot.clone(), msg.clone()).await?;
    }
    Ok(())
}

/// Get admin panel commands for bot command list
pub fn get_admin_panel_commands() -> Vec<teloxide::types::BotCommand> {
    AdminPanelCommand::bot_commands()
}

/// Example of how to set up bot commands including admin panel commands
pub async fn setup_bot_commands(bot: &Bot) -> Result<()> {
    // Get existing admin commands
    let existing_commands = crate::admin_handlers::AdminCommand::bot_commands();
    
    // Get admin panel commands
    let admin_panel_commands = get_admin_panel_commands();
    
    // Combine all commands
    let mut all_commands = existing_commands;
    all_commands.extend(admin_panel_commands);
    
    // Set bot commands
    bot.set_my_commands(all_commands).scope(BotCommandScope::Default).await?;
    
    Ok(())
}

/// Enhanced message handler that checks emergency stop status
pub async fn emergency_aware_message_handler(bot: Bot, msg: Message) -> Result<()> {
    // For now, use the standard integrated message handler
    // Emergency stop checking can be implemented later when needed
    integrated_message_handler(bot, msg).await
}

/// Initialize admin panel integration
pub async fn initialize_admin_panel_integration(bot: &Bot) -> Result<()> {
    // Set up bot commands
    setup_bot_commands(bot).await?;
    
    // Log initialization
    let mut redis_conn = match crate::get_redis_connection().await {
        Ok(conn) => conn,
        Err(e) => {
            log::warn!("Failed to get Redis connection during admin panel initialization: {}", e);
            return Ok(());
        }
    };
    if crate::admin_panel::auth::is_admin_panel_setup(&mut redis_conn).await? {
        log::info!("Admin panel integration initialized - admin panel is set up");
    } else {
        log::info!("Admin panel integration initialized - admin panel not yet set up");
    }
    
    Ok(())
}
