//! Admin Panel Commands

use anyhow::Result;
use redis::AsyncCommands;
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
    config::key,
    permissions::{AdminPermission, PermissionGroup},
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
    
    // TODO: Implement real dashboard with statistics
    // For now, show basic info
    let admin_users = get_all_admin_users(redis_conn).await?;
    let monitored_chats: Vec<String> = redis_conn.smembers(key::ADMIN_PANEL_MONITORED_CHATS_KEY).await?;
    
    let mut message = "📊 **Admin Panel Dashboard**\n\n".to_string();
    message.push_str(&format!("👥 Admin Members: {}\n", admin_users.len()));
    message.push_str(&format!("📋 Monitored Chats: {}\n", monitored_chats.len()));
    message.push_str(&format!("🕒 Last Updated: {}\n", chrono::Utc::now().format("%Y-%m-%d %H:%M:%S")));
    
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
    
    // Store setting in Redis
    redis_conn.hset(key::ADMIN_PANEL_SETTINGS_KEY, setting.clone(), value.clone()).await?;
    
    bot.send_message(
        chat.id,
        format!("✅ Updated setting `{}` to `{}`", setting, value),
    )
    .await?;
    
    Ok(())
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
    
    // TODO: Implement audit log functionality
    bot.send_message(
        chat.id,
        "📋 Audit log functionality is coming soon!",
    )
    .await?;
    
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
    
    // TODO: Implement emergency stop functionality
    bot.send_message(
        chat.id,
        "🛑 Emergency stop functionality is coming soon!",
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
    
    // TODO: Implement resume monitoring functionality
    bot.send_message(
        chat.id,
        "▶️ Resume monitoring functionality is coming soon!",
    )
    .await?;
    
    Ok(())
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
• `/auditlog [hours]` - Show audit log
• `/emergencystop` - Emergency stop all monitoring
• `/resumemonitoring` - Resume all monitoring

**Help:**
• `/help` - Show this help message

**Permission Groups:**
• Viewer: View statistics and audit logs
• Moderator: Manage chats and view statistics
• Manager: Manage users and configure settings
• Administrator: Full access to all features

**Permission Names:**
• `view_stats` - View statistics and dashboard
• `manage_chats` - Manage monitored chats
• `manage_users` - Manage admin panel users
• `configure_bot` - Configure bot settings
• `view_audit_log` - View audit logs
• `emergency_control` - Emergency control
• `full_access` - Full access to all features"#;
    
    bot.send_message(chat.id, help_text).await?;
    
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
