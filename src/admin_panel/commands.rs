//! Admin Panel Commands

use anyhow::Result;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use teloxide::{
    prelude::*,
    types::{Chat, ChatId, Message, User, UserId},
    utils::command::BotCommands,
    Bot,
};

use crate::admin_panel::{
    auth::{
        add_admin_user, get_admin_panel_chat_id, get_admin_panel_status, get_all_admin_users,
        has_permission, is_admin_panel_admin, is_admin_panel_member, is_admin_panel_setup,
        remove_admin_user, setup_admin_panel, update_admin_permissions,
    },
    config::{key, settings},
    permissions::{AdminPermission, AdminUser, PermissionGroup},
};

/// **Admin Panel Commands:** comprehensive admin panel management commands.
#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "Admin Panel Commands:")]
pub enum AdminPanelCommand {
    #[command(description = "Initialize admin panel in current chat")]
    SetupAdminPanel,
    
    #[command(description = "Add user to admin panel")]
    AddAdmin { username: String },
    
    #[command(description = "Remove user from admin panel")]
    RemoveAdmin { username: String },
    
    #[command(description = "List all admin panel members")]
    ListAdmins,
    
    #[command(description = "Set user permissions")]
    SetPermissions { username: String, permissions: String },
    
    #[command(description = "Show admin panel status")]
    PanelStatus,
    
    #[command(description = "Show all monitored chats")]
    MonitoredChats,
    
    #[command(description = "Add chat to monitoring")]
    AddChat { chat_id: String },
    
    #[command(description = "Remove chat from monitoring")]
    RemoveChat { chat_id: String },
    
    #[command(description = "Show real-time statistics dashboard")]
    Dashboard,
    
    #[command(description = "Configure bot settings")]
    Configure { setting: String, value: String },
    
    #[command(description = "Show audit log")]
    AuditLog { hours: Option<u32> },
    
    #[command(description = "Emergency stop all monitoring")]
    EmergencyStop,
    
    #[command(description = "Resume all monitoring")]
    ResumeMonitoring,
    
    #[command(description = "Show help for admin panel commands")]
    Help,
    
    #[command(description = "Show current bot configuration")]
    ShowConfig,
}

/// Handle admin panel commands
pub async fn handle_admin_panel_command(
    bot: Bot,
    msg: Message,
    command: AdminPanelCommand,
) -> Result<()> {
    let chat = msg.chat.clone();
    let user = msg.from().unwrap();
    
    // Get Redis connection
    let mut redis_conn = crate::get_redis_connection().await?;
    
    // Check if admin panel is set up (except for setup command)
    match command {
        AdminPanelCommand::SetupAdminPanel => {
            handle_setup_admin_panel(bot, msg, &mut redis_conn).await?;
        }
        _ => {
            if !is_admin_panel_setup(&mut redis_conn).await? {
                bot.send_message(
                    chat.id,
                    "❌ Admin panel is not set up. Use /setupadminpanel to initialize it.",
                )
                .await?;
                return Ok(());
            }
            
            // Check if user is admin panel member
            if !is_admin_panel_member(&mut redis_conn, user.id).await? {
                bot.send_message(
                    chat.id,
                    "❌ You are not a member of the admin panel. Contact an administrator.",
                )
                .await?;
                return Ok(());
            }
            
            // Handle other commands
            match command {
                AdminPanelCommand::AddAdmin { username } => {
                    handle_add_admin(bot, msg, &mut redis_conn, username).await?;
                }
                AdminPanelCommand::RemoveAdmin { username } => {
                    handle_remove_admin(bot, msg, &mut redis_conn, username).await?;
                }
                AdminPanelCommand::ListAdmins => {
                    handle_list_admins(bot, msg, &mut redis_conn).await?;
                }
                AdminPanelCommand::SetPermissions { username, permissions } => {
                    handle_set_permissions(bot, msg, &mut redis_conn, username, permissions).await?;
                }
                AdminPanelCommand::PanelStatus => {
                    handle_panel_status(bot, msg, &mut redis_conn).await?;
                }
                AdminPanelCommand::MonitoredChats => {
                    handle_monitored_chats(bot, msg, &mut redis_conn).await?;
                }
                AdminPanelCommand::AddChat { chat_id } => {
                    handle_add_chat(bot, msg, &mut redis_conn, chat_id).await?;
                }
                AdminPanelCommand::RemoveChat { chat_id } => {
                    handle_remove_chat(bot, msg, &mut redis_conn, chat_id).await?;
                }
                AdminPanelCommand::Dashboard => {
                    handle_dashboard(bot, msg, &mut redis_conn).await?;
                }
                AdminPanelCommand::Configure { setting, value } => {
                    handle_configure(bot, msg, &mut redis_conn, setting, value).await?;
                }
                AdminPanelCommand::AuditLog { hours } => {
                    handle_audit_log(bot, msg, &mut redis_conn, hours).await?;
                }
                AdminPanelCommand::EmergencyStop => {
                    handle_emergency_stop(bot, msg, &mut redis_conn).await?;
                }
                AdminPanelCommand::ResumeMonitoring => {
                    handle_resume_monitoring(bot, msg, &mut redis_conn).await?;
                }
                AdminPanelCommand::Help => {
                    handle_help(bot, msg).await?;
                }
                AdminPanelCommand::ShowConfig => {
                    handle_show_config(bot, msg, &mut redis_conn).await?;
                }
                _ => {}
            }
        }
    }
    
    Ok(())
}

/// Handle setup admin panel command
async fn handle_setup_admin_panel(
    bot: Bot,
    msg: Message,
    redis_conn: &mut redis::Connection,
) -> Result<()> {
    let chat = msg.chat.clone();
    let user = msg.from().unwrap();
    
    // Check if admin panel is already set up
    if is_admin_panel_setup(redis_conn).await? {
        bot.send_message(
            chat.id,
            "❌ Admin panel is already set up. Use /panelstatus to check the current status.",
        )
        .await?;
        return Ok(());
    }
    
    // Check if user is admin in this chat
    if !is_admin_panel_admin(&bot, redis_conn, chat.clone(), user.id).await? {
        bot.send_message(
            chat.id,
            "❌ You must be an administrator in this chat to set up the admin panel.",
        )
        .await?;
        return Ok(());
    }
    
    // Set up admin panel
    setup_admin_panel(redis_conn, chat.id, user).await?;
    
    bot.send_message(
        chat.id,
        format!(
            "✅ Admin panel has been set up successfully!\n\n\
            Chat ID: {}\n\
            Creator: {}\n\n\
            You now have full access to all admin panel features. Use /help to see available commands.",
            chat.id.0,
            user.full_name()
        ),
    )
    .await?;
    
    Ok(())
}

/// Handle add admin command
async fn handle_add_admin(
    bot: Bot,
    msg: Message,
    redis_conn: &mut redis::Connection,
    username: String,
) -> Result<()> {
    let chat = msg.chat.clone();
    let user = msg.from().unwrap();
    
    // Check if user has permission to manage users
    if !has_permission(redis_conn, user.id, &AdminPermission::ManageUsers).await? {
        bot.send_message(
            chat.id,
            "❌ You don't have permission to manage admin panel users.",
        )
        .await?;
        return Ok(());
    }
    
    // Try to resolve username to user
    let target_user = match resolve_username(&bot, &username).await? {
        Some(user) => user,
        None => {
            bot.send_message(
                chat.id,
                format!("❌ Could not find user with username: {}", username),
            )
            .await?;
            return Ok(());
        }
    };
    
    // Add user to admin panel with default permissions
    match add_admin_user(redis_conn, &target_user, user.id, Some(PermissionGroup::Viewer)).await {
        Ok(()) => {
            // Log the action
            add_audit_log_entry(
                redis_conn,
                user.id,
                user.full_name(),
                "Add Admin User".to_string(),
                Some(format!("Added {} (@{}) with Viewer permissions", 
                    target_user.full_name(), 
                    target_user.username.as_deref().unwrap_or("no_username"))),
            ).await?;
            
            bot.send_message(
                chat.id,
                format!(
                    "✅ Added {} to admin panel with Viewer permissions.\n\n\
                    Use /setpermissions to modify their permissions.",
                    target_user.full_name()
                ),
            )
            .await?;
        }
        Err(e) => {
            bot.send_message(
                chat.id,
                format!("❌ Failed to add user to admin panel: {}", e),
            )
            .await?;
        }
    }
    
    Ok(())
}

/// Handle remove admin command
async fn handle_remove_admin(
    bot: Bot,
    msg: Message,
    redis_conn: &mut redis::Connection,
    username: String,
) -> Result<()> {
    let chat = msg.chat.clone();
    let user = msg.from().unwrap();
    
    // Check if user has permission to manage users
    if !has_permission(redis_conn, user.id, &AdminPermission::ManageUsers).await? {
        bot.send_message(
            chat.id,
            "❌ You don't have permission to manage admin panel users.",
        )
        .await?;
        return Ok(());
    }
    
    // Try to resolve username to user
    let target_user = match resolve_username(&bot, &username).await? {
        Some(user) => user,
        None => {
            bot.send_message(
                chat.id,
                format!("❌ Could not find user with username: {}", username),
            )
            .await?;
            return Ok(());
        }
    };
    
    // Check if user is trying to remove themselves
    if target_user.id == user.id {
        bot.send_message(
            chat.id,
            "❌ You cannot remove yourself from the admin panel.",
        )
        .await?;
        return Ok(());
    }
    
    // Remove user from admin panel
    match remove_admin_user(redis_conn, target_user.id).await {
        Ok(()) => {
            // Log the action
            add_audit_log_entry(
                redis_conn,
                user.id,
                user.full_name(),
                "Remove Admin User".to_string(),
                Some(format!("Removed {} (@{}) from admin panel", 
                    target_user.full_name(), 
                    target_user.username.as_deref().unwrap_or("no_username"))),
            ).await?;
            
            bot.send_message(
                chat.id,
                format!("✅ Removed {} from admin panel.", target_user.full_name()),
            )
            .await?;
        }
        Err(e) => {
            bot.send_message(
                chat.id,
                format!("❌ Failed to remove user from admin panel: {}", e),
            )
            .await?;
        }
    }
    
    Ok(())
}

/// Handle list admins command
async fn handle_list_admins(
    bot: Bot,
    msg: Message,
    redis_conn: &mut redis::Connection,
) -> Result<()> {
    let chat = msg.chat.clone();
    let user = msg.from().unwrap();
    
    // Check if user has permission to view stats
    if !has_permission(redis_conn, user.id, &AdminPermission::ViewStats).await? {
        bot.send_message(
            chat.id,
            "❌ You don't have permission to view admin panel information.",
        )
        .await?;
        return Ok(());
    }
    
    // Get all admin users
    let admin_users = get_all_admin_users(redis_conn).await?;
    
    if admin_users.is_empty() {
        bot.send_message(chat.id, "📋 No admin panel members found.").await?;
        return Ok(());
    }
    
    // Build admin list message
    let mut message = "📋 **Admin Panel Members:**\n\n".to_string();
    
    for admin in admin_users {
        let username = admin.username.as_deref().unwrap_or("No username");
        let permissions: Vec<String> = admin.permissions.iter().map(|p| p.to_string()).collect();
        let permissions_str = if permissions.is_empty() {
            "No permissions".to_string()
        } else {
            permissions.join(", ")
        };
        
        message.push_str(&format!(
            "👤 **{}** (@{})\n",
            admin.display_name, username
        ));
        message.push_str(&format!("🆔 ID: `{}`\n", admin.user_id.0));
        message.push_str(&format!("🔑 Permissions: {}\n", permissions_str));
        message.push_str(&format!("📅 Added: {}\n", admin.added_at.format("%Y-%m-%d %H:%M")));
        if let Some(last_activity) = admin.last_activity {
            message.push_str(&format!("🕒 Last activity: {}\n", last_activity.format("%Y-%m-%d %H:%M")));
        }
        message.push_str("\n");
    }
    
    bot.send_message(chat.id, message).await?;
    
    Ok(())
}

/// Handle set permissions command
async fn handle_set_permissions(
    bot: Bot,
    msg: Message,
    redis_conn: &mut redis::Connection,
    username: String,
    permissions: String,
) -> Result<()> {
    let chat = msg.chat.clone();
    let user = msg.from().unwrap();
    
    // Check if user has permission to manage users
    if !has_permission(redis_conn, user.id, &AdminPermission::ManageUsers).await? {
        bot.send_message(
            chat.id,
            "❌ You don't have permission to manage admin panel users.",
        )
        .await?;
        return Ok(());
    }
    
    // Try to resolve username to user
    let target_user = match resolve_username(&bot, &username).await? {
        Some(user) => user,
        None => {
            bot.send_message(
                chat.id,
                format!("❌ Could not find user with username: {}", username),
            )
            .await?;
            return Ok(());
        }
    };
    
    // Parse permissions
    let permission_list: Vec<AdminPermission> = permissions
        .split(',')
        .map(|s| s.trim())
        .filter_map(|s| AdminPermission::from_string(s))
        .collect();
    
    if permission_list.is_empty() {
        bot.send_message(
            chat.id,
            "❌ No valid permissions specified. Use comma-separated values like: view_stats,manage_chats",
        )
        .await?;
        return Ok(());
    }
    
    // Update permissions
    match update_admin_permissions(redis_conn, target_user.id, permission_list.clone()).await {
        Ok(()) => {
            let permissions_str: Vec<String> = permission_list.iter().map(|p| p.to_string()).collect();
            
            // Log the action
            add_audit_log_entry(
                redis_conn,
                user.id,
                user.full_name(),
                "Update Permissions".to_string(),
                Some(format!("Updated permissions for {} (@{}): {}", 
                    target_user.full_name(), 
                    target_user.username.as_deref().unwrap_or("no_username"),
                    permissions_str.join(", "))),
            ).await?;
            
            bot.send_message(
                chat.id,
                format!(
                    "✅ Updated permissions for {}:\n{}",
                    target_user.full_name(),
                    permissions_str.join(", ")
                ),
            )
            .await?;
        }
        Err(e) => {
            bot.send_message(
                chat.id,
                format!("❌ Failed to update permissions: {}", e),
            )
            .await?;
        }
    }
    
    Ok(())
}

/// Handle panel status command
async fn handle_panel_status(
    bot: Bot,
    msg: Message,
    redis_conn: &mut redis::Connection,
) -> Result<()> {
    let chat = msg.chat.clone();
    let user = msg.from().unwrap();
    
    // Check if user has permission to view stats
    if !has_permission(redis_conn, user.id, &AdminPermission::ViewStats).await? {
        bot.send_message(
            chat.id,
            "❌ You don't have permission to view admin panel information.",
        )
        .await?;
        return Ok(());
    }
    
    let status = get_admin_panel_status(redis_conn).await?;
    let admin_chat_id = get_admin_panel_chat_id(redis_conn).await?;
    let admin_users = get_all_admin_users(redis_conn).await?;
    
    let mut message = format!("📊 **Admin Panel Status:** {}\n\n", status);
    
    if let Some(chat_id) = admin_chat_id {
        message.push_str(&format!("🏠 Admin Panel Chat: `{}`\n", chat_id.0));
    }
    
    message.push_str(&format!("👥 Total Members: {}\n", admin_users.len()));
    
    // Count permissions
    let mut permission_counts = std::collections::HashMap::new();
    for admin in &admin_users {
        for permission in &admin.permissions {
            *permission_counts.entry(permission).or_insert(0) += 1;
        }
    }
    
    if !permission_counts.is_empty() {
        message.push_str("\n🔑 **Permission Distribution:**\n");
        for (permission, count) in permission_counts {
            message.push_str(&format!("• {}: {}\n", permission, count));
        }
    }
    
    bot.send_message(chat.id, message).await?;
    
    Ok(())
}

/// Handle monitored chats command
async fn handle_monitored_chats(
    bot: Bot,
    msg: Message,
    redis_conn: &mut redis::Connection,
) -> Result<()> {
    let chat = msg.chat.clone();
    let user = msg.from().unwrap();
    
    // Check if user has permission to manage chats
    if !has_permission(redis_conn, user.id, &AdminPermission::ManageChats).await? {
        bot.send_message(
            chat.id,
            "❌ You don't have permission to manage monitored chats.",
        )
        .await?;
        return Ok(());
    }
    
    // Get monitored chats
    let monitored_chats: Vec<String> = redis_conn.smembers(key::ADMIN_PANEL_MONITORED_CHATS_KEY).await?;
    
    if monitored_chats.is_empty() {
        bot.send_message(chat.id, "📋 No monitored chats found.").await?;
        return Ok(());
    }
    
    let mut message = "📋 **Monitored Chats:**\n\n".to_string();
    
    for chat_id_str in monitored_chats {
        message.push_str(&format!("• `{}`\n", chat_id_str));
    }
    
    bot.send_message(chat.id, message).await?;
    
    Ok(())
}

/// Handle add chat command
async fn handle_add_chat(
    bot: Bot,
    msg: Message,
    redis_conn: &mut redis::Connection,
    chat_id: String,
) -> Result<()> {
    let chat = msg.chat.clone();
    let user = msg.from().unwrap();
    
    // Check if user has permission to manage chats
    if !has_permission(redis_conn, user.id, &AdminPermission::ManageChats).await? {
        bot.send_message(
            chat.id,
            "❌ You don't have permission to manage monitored chats.",
        )
        .await?;
        return Ok(());
    }
    
    // Parse chat ID
    let chat_id_parsed = match chat_id.parse::<i64>() {
        Ok(id) => ChatId(id),
        Err(_) => {
            bot.send_message(
                chat.id,
                "❌ Invalid chat ID format. Please provide a valid numeric chat ID.",
            )
            .await?;
            return Ok(());
        }
    };
    
    // Add chat to monitoring
    redis_conn.sadd(key::ADMIN_PANEL_MONITORED_CHATS_KEY, chat_id).await?;
    
    // Log the action
    add_audit_log_entry(
        redis_conn,
        user.id,
        user.full_name(),
        "Add Monitored Chat".to_string(),
        Some(format!("Added chat {} to monitoring", chat_id_parsed.0)),
    ).await?;
    
    bot.send_message(
        chat.id,
        format!("✅ Added chat `{}` to monitoring.", chat_id_parsed.0),
    )
    .await?;
    
    Ok(())
}

/// Handle remove chat command
async fn handle_remove_chat(
    bot: Bot,
    msg: Message,
    redis_conn: &mut redis::Connection,
    chat_id: String,
) -> Result<()> {
    let chat = msg.chat.clone();
    let user = msg.from().unwrap();
    
    // Check if user has permission to manage chats
    if !has_permission(redis_conn, user.id, &AdminPermission::ManageChats).await? {
        bot.send_message(
            chat.id,
            "❌ You don't have permission to manage monitored chats.",
        )
        .await?;
        return Ok(());
    }
    
    // Remove chat from monitoring
    redis_conn.srem(key::ADMIN_PANEL_MONITORED_CHATS_KEY, &chat_id).await?;
    
    // Log the action
    add_audit_log_entry(
        redis_conn,
        user.id,
        user.full_name(),
        "Remove Monitored Chat".to_string(),
        Some(format!("Removed chat {} from monitoring", chat_id)),
    ).await?;
    
    bot.send_message(
        chat.id,
        format!("✅ Removed chat `{}` from monitoring.", chat_id),
    )
    .await?;
    
    Ok(())
}

/// Handle dashboard command
async fn handle_dashboard(
    bot: Bot,
    msg: Message,
    redis_conn: &mut redis::Connection,
) -> Result<()> {
    let chat = msg.chat.clone();
    let user = msg.from().unwrap();
    
    // Check if user has permission to view stats
    if !has_permission(redis_conn, user.id, &AdminPermission::ViewStats).await? {
        bot.send_message(
            chat.id,
            "❌ You don't have permission to view the dashboard.",
        )
        .await?;
        return Ok(());
    }
    
    // Get comprehensive dashboard data
    let admin_users = get_all_admin_users(redis_conn).await?;
    let monitored_chats: Vec<String> = redis_conn.smembers(key::ADMIN_PANEL_MONITORED_CHATS_KEY).await?;
    
    // Get bot statistics from Redis
    let total_users = get_total_users(redis_conn).await?;
    let total_chats = get_total_chats(redis_conn).await?;
    let recent_spam_events = get_recent_spam_events(redis_conn).await?;
    let system_health = get_system_health(redis_conn).await?;
    
    // Build comprehensive dashboard message
    let mut message = "📊 **Admin Panel Dashboard**\n\n".to_string();
    
    // Admin Panel Status
    message.push_str("🏠 **Admin Panel Status:**\n");
    message.push_str(&format!("• Members: {}\n", admin_users.len()));
    message.push_str(&format!("• Monitored Chats: {}\n", monitored_chats.len()));
    message.push_str(&format!("• Status: {}\n", get_admin_panel_status(redis_conn).await?));
    message.push_str("\n");
    
    // Bot Statistics
    message.push_str("🤖 **Bot Statistics:**\n");
    message.push_str(&format!("• Total Users: {}\n", total_users));
    message.push_str(&format!("• Total Chats: {}\n", total_chats));
    message.push_str(&format!("• Recent Spam Events: {}\n", recent_spam_events));
    message.push_str("\n");
    
    // System Health
    message.push_str("💚 **System Health:**\n");
    message.push_str(&format!("• Redis Connection: {}\n", system_health.redis_status));
    message.push_str(&format!("• Bayes Classifier: {}\n", system_health.bayes_status));
    message.push_str(&format!("• Active Filters: {}\n", system_health.active_filters));
    message.push_str("\n");
    
    // Recent Activity
    message.push_str("🕒 **Recent Activity:**\n");
    message.push_str(&format!("• Last Updated: {}\n", chrono::Utc::now().format("%Y-%m-%d %H:%M:%S")));
    
    // Add admin activity if any
    let recent_admins: Vec<&AdminUser> = admin_users.iter()
        .filter(|admin| admin.last_activity.is_some())
        .collect();
    
    if !recent_admins.is_empty() {
        message.push_str("• Recent Admin Activity:\n");
        for admin in recent_admins.iter().take(3) {
            if let Some(last_activity) = admin.last_activity {
                let time_ago = chrono::Utc::now() - last_activity;
                let minutes_ago = time_ago.num_minutes();
                message.push_str(&format!("  - {}: {} minutes ago\n", 
                    admin.display_name, minutes_ago));
            }
        }
    }
    
    bot.send_message(chat.id, message).await?;
    
    Ok(())
}

/// Handle configure command
async fn handle_configure(
    bot: Bot,
    msg: Message,
    redis_conn: &mut redis::Connection,
    setting: String,
    value: String,
) -> Result<()> {
    let chat = msg.chat.clone();
    let user = msg.from().unwrap();
    
    // Check if user has permission to configure bot
    if !has_permission(redis_conn, user.id, &AdminPermission::ConfigureBot).await? {
        bot.send_message(
            chat.id,
            "❌ You don't have permission to configure bot settings.",
        )
        .await?;
        return Ok(());
    }
    
    // Validate and process the setting
    match validate_and_set_config(redis_conn, &setting, &value).await {
        Ok(result) => {
            // Log the configuration change
            add_audit_log_entry(
                redis_conn,
                user.id,
                user.full_name(),
                "Configuration Change".to_string(),
                Some(format!("Setting '{}' changed to '{}'", setting, value)),
            ).await?;
            
            bot.send_message(
                chat.id,
                format!("✅ **Configuration Updated**\n\nSetting: `{}`\nValue: `{}`\n\n{}", 
                    setting, value, result),
            )
            .await?;
        }
        Err(e) => {
            bot.send_message(
                chat.id,
                format!("❌ **Configuration Error**\n\nSetting: `{}`\nValue: `{}`\n\nError: {}", 
                    setting, value, e),
            )
            .await?;
        }
    }
    
    Ok(())
}

// Configuration validation and setting
async fn validate_and_set_config(
    redis_conn: &mut redis::Connection,
    setting: &str,
    value: &str,
) -> Result<String> {
    match setting.to_lowercase().as_str() {
        "spam_threshold" => {
            let threshold = value.parse::<f64>()
                .map_err(|_| anyhow::anyhow!("Spam threshold must be a number between 0.0 and 1.0"))?;
            
            if threshold < 0.0 || threshold > 1.0 {
                return Err(anyhow::anyhow!("Spam threshold must be between 0.0 and 1.0"));
            }
            
            redis_conn.hset(key::ADMIN_PANEL_SETTINGS_KEY, "spam_threshold", threshold.to_string()).await?;
            Ok("Spam detection threshold updated".to_string())
        }
        
        "reputation_decay_rate" => {
            let rate = value.parse::<i64>()
                .map_err(|_| anyhow::anyhow!("Reputation decay rate must be a positive integer"))?;
            
            if rate < 0 {
                return Err(anyhow::anyhow!("Reputation decay rate must be positive"));
            }
            
            redis_conn.hset(key::ADMIN_PANEL_SETTINGS_KEY, "reputation_decay_rate", rate.to_string()).await?;
            Ok("Reputation decay rate updated".to_string())
        }
        
        "max_ban_duration" => {
            let duration = value.parse::<u64>()
                .map_err(|_| anyhow::anyhow!("Max ban duration must be a positive integer (hours)"))?;
            
            if duration == 0 {
                return Err(anyhow::anyhow!("Max ban duration must be greater than 0"));
            }
            
            redis_conn.hset(key::ADMIN_PANEL_SETTINGS_KEY, "max_ban_duration", duration.to_string()).await?;
            Ok("Maximum ban duration updated".to_string())
        }
        
        "auto_ban_enabled" => {
            let enabled = match value.to_lowercase().as_str() {
                "true" | "yes" | "1" | "on" => true,
                "false" | "no" | "0" | "off" => false,
                _ => return Err(anyhow::anyhow!("Auto ban enabled must be true/false, yes/no, 1/0, or on/off")),
            };
            
            redis_conn.hset(key::ADMIN_PANEL_SETTINGS_KEY, "auto_ban_enabled", enabled.to_string()).await?;
            Ok(format!("Auto ban {}", if enabled { "enabled" } else { "disabled" }))
        }
        
        "bayes_learning_enabled" => {
            let enabled = match value.to_lowercase().as_str() {
                "true" | "yes" | "1" | "on" => true,
                "false" | "no" | "0" | "off" => false,
                _ => return Err(anyhow::anyhow!("Bayes learning must be true/false, yes/no, 1/0, or on/off")),
            };
            
            redis_conn.hset(key::ADMIN_PANEL_SETTINGS_KEY, "bayes_learning_enabled", enabled.to_string()).await?;
            Ok(format!("Bayes learning {}", if enabled { "enabled" } else { "disabled" }))
        }
        
        "notification_level" => {
            let level = match value.to_lowercase().as_str() {
                "all" | "high" | "medium" | "low" | "none" => value.to_lowercase(),
                _ => return Err(anyhow::anyhow!("Notification level must be: all, high, medium, low, or none")),
            };
            
            redis_conn.hset(key::ADMIN_PANEL_SETTINGS_KEY, "notification_level", level).await?;
            Ok("Notification level updated".to_string())
        }
        
        "maintenance_mode" => {
            let enabled = match value.to_lowercase().as_str() {
                "true" | "yes" | "1" | "on" => true,
                "false" | "no" | "0" | "off" => false,
                _ => return Err(anyhow::anyhow!("Maintenance mode must be true/false, yes/no, 1/0, or on/off")),
            };
            
            redis_conn.hset(key::ADMIN_PANEL_SETTINGS_KEY, "maintenance_mode", enabled.to_string()).await?;
            Ok(format!("Maintenance mode {}", if enabled { "enabled" } else { "disabled" }))
        }
        
        _ => {
            // For unknown settings, store as-is but warn
            redis_conn.hset(key::ADMIN_PANEL_SETTINGS_KEY, setting, value).await?;
            Ok(format!("Unknown setting '{}' stored (no validation performed)", setting))
        }
    }
}

// Helper function to get configuration value
pub async fn get_config_value(
    redis_conn: &mut redis::Connection,
    setting: &str,
) -> Result<Option<String>> {
    let value: Option<String> = redis_conn.hget(key::ADMIN_PANEL_SETTINGS_KEY, setting).await?;
    Ok(value)
}

// Helper function to get all configuration
pub async fn get_all_config(redis_conn: &mut redis::Connection) -> Result<std::collections::HashMap<String, String>> {
    let config: std::collections::HashMap<String, String> = redis_conn.hgetall(key::ADMIN_PANEL_SETTINGS_KEY).await?;
    Ok(config)
}

/// Handle audit log command
async fn handle_audit_log(
    bot: Bot,
    msg: Message,
    redis_conn: &mut redis::Connection,
    hours: Option<u32>,
) -> Result<()> {
    let chat = msg.chat.clone();
    let user = msg.from().unwrap();
    
    // Check if user has permission to view audit log
    if !has_permission(redis_conn, user.id, &AdminPermission::ViewAuditLog).await? {
        bot.send_message(
            chat.id,
            "❌ You don't have permission to view audit logs.",
        )
        .await?;
        return Ok(());
    }
    
    // Get audit log entries
    let hours_to_show = hours.unwrap_or(24); // Default to last 24 hours
    let audit_entries = get_audit_log_entries(redis_conn, hours_to_show).await?;
    
    if audit_entries.is_empty() {
        bot.send_message(
            chat.id,
            format!("📋 No audit log entries found for the last {} hours.", hours_to_show),
        )
        .await?;
        return Ok(());
    }
    
    // Build audit log message
    let mut message = format!("📋 **Audit Log (Last {} hours):**\n\n", hours_to_show);
    
    for entry in audit_entries.iter().take(20) { // Limit to 20 entries to avoid message length issues
        let timestamp = entry.timestamp.format("%Y-%m-%d %H:%M:%S");
        message.push_str(&format!("🕒 **{}**\n", timestamp));
        message.push_str(&format!("👤 User: {}\n", entry.user_name));
        message.push_str(&format!("🔧 Action: {}\n", entry.action));
        if let Some(details) = &entry.details {
            message.push_str(&format!("📝 Details: {}\n", details));
        }
        message.push_str("\n");
    }
    
    if audit_entries.len() > 20 {
        message.push_str(&format!("... and {} more entries", audit_entries.len() - 20));
    }
    
    bot.send_message(chat.id, message).await?;
    
    Ok(())
}

// Audit log entry structure
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AuditLogEntry {
    timestamp: chrono::DateTime<chrono::Utc>,
    user_id: UserId,
    user_name: String,
    action: String,
    details: Option<String>,
}

// Helper function to get audit log entries
async fn get_audit_log_entries(
    redis_conn: &mut redis::Connection,
    hours: u32,
) -> Result<Vec<AuditLogEntry>> {
    let cutoff_time = chrono::Utc::now() - chrono::Duration::hours(hours as i64);
    
    // Get all audit log entries from Redis
    let entries: Vec<String> = redis_conn.lrange(key::ADMIN_PANEL_AUDIT_LOG_KEY, 0, -1).await?;
    let mut audit_entries = Vec::new();
    
    for entry_str in entries {
        if let Ok(entry) = serde_json::from_str::<AuditLogEntry>(&entry_str) {
            if entry.timestamp >= cutoff_time {
                audit_entries.push(entry);
            }
        }
    }
    
    // Sort by timestamp (newest first)
    audit_entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    
    Ok(audit_entries)
}

// Helper function to add audit log entry
pub async fn add_audit_log_entry(
    redis_conn: &mut redis::Connection,
    user_id: UserId,
    user_name: String,
    action: String,
    details: Option<String>,
) -> Result<()> {
    let entry = AuditLogEntry {
        timestamp: chrono::Utc::now(),
        user_id,
        user_name,
        action,
        details,
    };
    
    let entry_json = serde_json::to_string(&entry)?;
    
    // Add to the beginning of the list (newest first)
    redis_conn.lpush(key::ADMIN_PANEL_AUDIT_LOG_KEY, entry_json).await?;
    
    // Trim the list to keep only the latest entries
    redis_conn.ltrim(key::ADMIN_PANEL_AUDIT_LOG_KEY, 0, settings::MAX_AUDIT_LOG_ENTRIES as isize - 1).await?;
    
    Ok(())
}

/// Handle emergency stop command
async fn handle_emergency_stop(
    bot: Bot,
    msg: Message,
    redis_conn: &mut redis::Connection,
) -> Result<()> {
    let chat = msg.chat.clone();
    let user = msg.from().unwrap();
    
    // Check if user has permission for emergency control
    if !has_permission(redis_conn, user.id, &AdminPermission::EmergencyControl).await? {
        bot.send_message(
            chat.id,
            "❌ You don't have permission to use emergency controls.",
        )
        .await?;
        return Ok(());
    }
    
    // Set emergency stop flag in Redis
    redis_conn.set("admin:emergency_stop", "true").await?;
    redis_conn.set("admin:emergency_stop_timestamp", chrono::Utc::now().timestamp().to_string()).await?;
    redis_conn.set("admin:emergency_stop_by", user.id.0.to_string()).await?;
    
    // Log the emergency stop action
    add_audit_log_entry(
        redis_conn,
        user.id,
        user.full_name(),
        "Emergency Stop".to_string(),
        Some("All monitoring has been stopped".to_string()),
    ).await?;
    
    bot.send_message(
        chat.id,
        "🛑 **EMERGENCY STOP ACTIVATED**\n\nAll monitoring has been stopped. Use /resumemonitoring to resume normal operations.",
    )
    .await?;
    
    Ok(())
}

/// Handle resume monitoring command
async fn handle_resume_monitoring(
    bot: Bot,
    msg: Message,
    redis_conn: &mut redis::Connection,
) -> Result<()> {
    let chat = msg.chat.clone();
    let user = msg.from().unwrap();
    
    // Check if user has permission for emergency control
    if !has_permission(redis_conn, user.id, &AdminPermission::EmergencyControl).await? {
        bot.send_message(
            chat.id,
            "❌ You don't have permission to use emergency controls.",
        )
        .await?;
        return Ok(());
    }
    
    // Check if emergency stop is active
    let emergency_stop: Option<String> = redis_conn.get("admin:emergency_stop").await?;
    
    if emergency_stop.is_none() {
        bot.send_message(
            chat.id,
            "ℹ️ No emergency stop is currently active.",
        )
        .await?;
        return Ok(());
    }
    
    // Remove emergency stop flag
    redis_conn.del("admin:emergency_stop").await?;
    redis_conn.del("admin:emergency_stop_timestamp").await?;
    redis_conn.del("admin:emergency_stop_by").await?;
    
    // Log the resume action
    add_audit_log_entry(
        redis_conn,
        user.id,
        user.full_name(),
        "Resume Monitoring".to_string(),
        Some("All monitoring has been resumed".to_string()),
    ).await?;
    
    bot.send_message(
        chat.id,
        "▶️ **MONITORING RESUMED**\n\nAll monitoring has been resumed. Normal operations are now active.",
    )
    .await?;
    
    Ok(())
}

// Helper function to check if emergency stop is active
pub async fn is_emergency_stop_active(redis_conn: &mut redis::Connection) -> Result<bool> {
    let emergency_stop: Option<String> = redis_conn.get("admin:emergency_stop").await?;
    Ok(emergency_stop.is_some())
}

// Helper function to get emergency stop info
pub async fn get_emergency_stop_info(redis_conn: &mut redis::Connection) -> Result<Option<EmergencyStopInfo>> {
    let emergency_stop: Option<String> = redis_conn.get("admin:emergency_stop").await?;
    
    if emergency_stop.is_some() {
        let timestamp_str: Option<String> = redis_conn.get("admin:emergency_stop_timestamp").await?;
        let user_id_str: Option<String> = redis_conn.get("admin:emergency_stop_by").await?;
        
        let timestamp = if let Some(ts_str) = timestamp_str {
            ts_str.parse::<i64>().ok().and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
        } else {
            None
        };
        
        let user_id = if let Some(uid_str) = user_id_str {
            uid_str.parse::<u64>().ok().map(UserId)
        } else {
            None
        };
        
        Ok(Some(EmergencyStopInfo {
            timestamp,
            user_id,
        }))
    } else {
        Ok(None)
    }
}

#[derive(Debug, Clone)]
pub struct EmergencyStopInfo {
    pub timestamp: Option<chrono::DateTime<chrono::Utc>>,
    pub user_id: Option<UserId>,
}

/// Handle help command
async fn handle_help(bot: Bot, msg: Message) -> Result<()> {
    let chat = msg.chat.clone();
    
    let help_text = r#"🤖 **Admin Panel Commands**

**Setup & Management:**
• `/setupadminpanel` - Initialize admin panel in current chat
• `/addadmin <username>` - Add user to admin panel
• `/removeadmin <username>` - Remove user from admin panel
• `/listadmins` - List all admin panel members
• `/setpermissions <username> <permissions>` - Set user permissions
• `/panelstatus` - Show admin panel status

**Chat Management:**
• `/monitoredchats` - Show all monitored chats
• `/addchat <chat_id>` - Add chat to monitoring
• `/removechat <chat_id>` - Remove chat from monitoring

**Monitoring & Control:**
• `/dashboard` - Show real-time statistics dashboard
• `/configure <setting> <value>` - Configure bot settings
• `/showconfig` - Show current bot configuration
• `/auditlog [hours]` - Show audit log (default: 24h)
• `/emergencystop` - Emergency stop all monitoring
• `/resumemonitoring` - Resume all monitoring

**Help:**
• `/help` - Show this help message

**Permission Groups:**
• Viewer: View statistics, audit logs, and configuration
• Moderator: Manage chats and view statistics
• Manager: Manage users, configure settings, and view all data
• Administrator: Full access to all features

**Permission Names:**
• `view_stats` - View statistics and dashboard
• `manage_chats` - Manage monitored chats
• `manage_users` - Manage admin panel users
• `configure_bot` - Configure bot settings
• `view_audit_log` - View audit logs
• `view_config` - View bot configuration
• `emergency_control` - Emergency control
• `full_access` - Full access to all features

**Configuration Settings:**
• `spam_threshold` - Spam detection threshold (0.0-1.0)
• `reputation_decay_rate` - Reputation decay rate (positive integer)
• `max_ban_duration` - Maximum ban duration in hours
• `auto_ban_enabled` - Enable/disable auto-banning (true/false)
• `bayes_learning_enabled` - Enable/disable Bayes learning (true/false)
• `notification_level` - Notification level (all/high/medium/low/none)
• `maintenance_mode` - Enable/disable maintenance mode (true/false)

**Examples:**
• `/configure spam_threshold 0.8`
• `/configure auto_ban_enabled true`
• `/configure notification_level high`
• `/auditlog 48` - Show last 48 hours of audit log"#;
    
    bot.send_message(chat.id, help_text).await?;
    
    Ok(())
}

/// Handle show config command
async fn handle_show_config(
    bot: Bot,
    msg: Message,
    redis_conn: &mut redis::Connection,
) -> Result<()> {
    let chat = msg.chat.clone();
    let user = msg.from().unwrap();
    
    // Check if user has permission to view config
    if !has_permission(redis_conn, user.id, &AdminPermission::ViewConfig).await? {
        bot.send_message(
            chat.id,
            "❌ You don't have permission to view bot configuration.",
        )
        .await?;
        return Ok(());
    }
    
    let config = get_all_config(redis_conn).await?;
    
    if config.is_empty() {
        bot.send_message(
            chat.id,
            "📋 No bot configuration found. Use `/configure <setting> <value>` to set one.",
        )
        .await?;
        return Ok(());
    }
    
    let mut message = "📋 **Bot Configuration:**\n\n".to_string();
    
    for (key, value) in config {
        message.push_str(&format!("• `{}`: `{}`\n", key, value));
    }
    
    bot.send_message(chat.id, message).await?;
    
    Ok(())
}

/// Helper function to resolve username to user
async fn resolve_username(bot: &Bot, username: &str) -> Result<Option<User>> {
    // Remove @ if present
    let clean_username = username.trim_start_matches('@');
    
    // For now, create a mock user for testing
    // In a real implementation, you would:
    // 1. Store user mappings in Redis when users interact with the bot
    // 2. Use Telegram's API to resolve usernames (requires additional API calls)
    // 3. Or require users to provide their user ID instead of username
    
    // Mock implementation for testing
    if clean_username.starts_with("test_") {
        // Create a mock user for testing purposes
        let user_id = clean_username.replace("test_", "").parse::<u64>().unwrap_or(123456789);
        Ok(Some(User {
            id: UserId(user_id),
            is_bot: false,
            first_name: clean_username.to_string(),
            last_name: None,
            username: Some(clean_username.to_string()),
            language_code: None,
            is_premium: false,
            added_to_attachment_menu: false,
        }))
    } else {
        // For now, return None for real usernames
        // TODO: Implement proper username resolution
        Ok(None)
    }
}

// Helper functions for dashboard statistics
async fn get_total_users(redis_conn: &mut redis::Connection) -> Result<usize> {
    let user_keys: Vec<String> = redis_conn.keys(format!("{}*", crate::config::key::TG_USERS_PREFIX)).await?;
    Ok(user_keys.len())
}

async fn get_total_chats(redis_conn: &mut redis::Connection) -> Result<usize> {
    let chat_keys: Vec<String> = redis_conn.keys(format!("{}*", crate::config::key::TG_CHATS_PREFIX)).await?;
    Ok(chat_keys.len())
}

async fn get_recent_spam_events(redis_conn: &mut redis::Connection) -> Result<usize> {
    // Count recent spam events (last 24 hours)
    let now = chrono::Utc::now();
    let yesterday = now - chrono::Duration::hours(24);
    
    let spam_keys: Vec<String> = redis_conn.keys("spam:*").await?;
    let mut recent_count = 0;
    
    for key in spam_keys {
        if let Ok(timestamp_str) = redis_conn.hget::<_, _, Option<String>>(&key, "timestamp").await {
            if let Some(ts_str) = timestamp_str {
                if let Ok(timestamp) = ts_str.parse::<i64>() {
                    let event_time = chrono::DateTime::from_timestamp(timestamp, 0)
                        .unwrap_or(chrono::Utc::now());
                    if event_time >= yesterday {
                        recent_count += 1;
                    }
                }
            }
        }
    }
    
    Ok(recent_count)
}

struct SystemHealth {
    redis_status: String,
    bayes_status: String,
    active_filters: usize,
}

async fn get_system_health(redis_conn: &mut redis::Connection) -> Result<SystemHealth> {
    // Check Redis connection
    let redis_status = match redis_conn.ping::<String>().await {
        Ok(_) => "✅ Connected".to_string(),
        Err(_) => "❌ Disconnected".to_string(),
    };
    
    // Check Bayes classifier status
    let bayes_status = match crate::bayes_manager::BayesManager::new() {
        Ok(bayes) => {
            match bayes.is_ready() {
                Ok(true) => "✅ Ready".to_string(),
                Ok(false) => "🔄 Training".to_string(),
                Err(_) => "❌ Error".to_string(),
            }
        }
        Err(_) => "❌ Not Available".to_string(),
    };
    
    // Count active filters
    let active_filters = get_active_filters_count(redis_conn).await?;
    
    Ok(SystemHealth {
        redis_status,
        bayes_status,
        active_filters,
    })
}

async fn get_active_filters_count(redis_conn: &mut redis::Connection) -> Result<usize> {
    // Count enabled features/filters
    let enabled_features: Vec<String> = redis_conn.smembers(crate::config::ENABLED_FEATURES_KEY).await?;
    Ok(enabled_features.len())
}
