//! Admin Panel Integration with Existing Bot System

use anyhow::Result;
use teloxide::{
    prelude::*,
    types::{Message, Update},
    utils::command::BotCommands,
};

use crate::admin_panel::commands::{AdminPanelCommand, handle_admin_panel_command};

/// Check if a message contains an admin panel command
pub fn is_admin_panel_command(text: &str) -> bool {
    AdminPanelCommand::parse(text, "rspamd-bot").is_ok()
}

/// Handle admin panel commands in the message handler
pub async fn handle_admin_panel_message(bot: Bot, msg: Message) -> Result<()> {
    if let Some(text) = msg.text() {
        if let Ok(cmd) = AdminPanelCommand::parse(text, "rspamd-bot") {
            handle_admin_panel_command(bot, msg, cmd).await?;
        }
    }
    Ok(())
}

/// Example integration with existing message handler
/// This function shows how to integrate admin panel commands with the existing bot system
pub async fn integrated_message_handler(bot: Bot, msg: Message) -> Result<()> {
    if let Some(text) = msg.text() {
        // First check if it's an admin panel command
        if is_admin_panel_command(text) {
            if let Ok(cmd) = AdminPanelCommand::parse(text, "rspamd-bot") {
                handle_admin_panel_command(bot.clone(), msg.clone(), cmd).await?;
                return Ok(());
            }
        }
        
        // If not an admin panel command, handle with existing admin commands
        if let Ok(cmd) = crate::admin_handlers::AdminCommand::parse(text, "rspamd-bot") {
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
    bot.set_my_commands(all_commands).await?;
    
    Ok(())
}
