//! Admin Panel Authentication System

use anyhow::Result;
use redis::{AsyncCommands, Commands};
use teloxide::{
    prelude::Requester,
    types::{Chat, ChatId, ChatMemberStatus, User, UserId},
    Bot,
};

use crate::admin_panel::{
    config::{key, AdminPanelStatus},
    permissions::{AdminPermission, AdminUser, PermissionGroup},
};

/// **Admin Panel Authentication:** handles admin panel access control.

/// Check if the admin panel is set up
pub async fn is_admin_panel_setup(redis_conn: &mut redis::Connection) -> Result<bool> {
    let chat_id: Option<String> = redis_conn.get(key::ADMIN_PANEL_CHAT_KEY)?;
    Ok(chat_id.is_some())
}

/// Get the admin panel chat ID
pub async fn get_admin_panel_chat_id(redis_conn: &mut redis::Connection) -> Result<Option<ChatId>> {
    let chat_id_str: Option<String> = redis_conn.get(key::ADMIN_PANEL_CHAT_KEY)?;
    match chat_id_str {
        Some(id_str) => {
            let chat_id = id_str.parse::<i64>()?;
            Ok(Some(ChatId(chat_id)))
        }
        None => Ok(None),
    }
}

/// Set up the admin panel in a chat
pub async fn setup_admin_panel(
    redis_conn: &mut redis::Connection,
    chat_id: ChatId,
    creator: &User,
) -> Result<()> {
    // Store the admin panel chat ID
    redis_conn.set(key::ADMIN_PANEL_CHAT_KEY, chat_id.0.to_string()).await?;
    
    // Add the creator as the first admin with full access
    let mut admin_user = AdminUser::new(
        creator.id,
        creator.username.clone(),
        creator.full_name(),
        creator.id,
    );
    
    // Give creator full access
    admin_user.add_permission(AdminPermission::FullAccess);
    
    // Add to admin panel members
    redis_conn.sadd(key::ADMIN_PANEL_MEMBERS_KEY, creator.id.0.to_string()).await?;
    
    // Store admin user data
    let admin_data = serde_json::to_string(&admin_user)?;
    redis_conn
        .hset(key::ADMIN_PANEL_PERMISSIONS_KEY, creator.id.0.to_string(), admin_data)
        .await?;
    
    Ok(())
}

/// Check if a user is a member of the admin panel
pub async fn is_admin_panel_member(
    redis_conn: &mut redis::Connection,
    user_id: UserId,
) -> Result<bool> {
    let is_member: bool = redis_conn.sismember(key::ADMIN_PANEL_MEMBERS_KEY, user_id.0.to_string()).await?;
    Ok(is_member)
}

/// Get admin user data from Redis
pub async fn get_admin_user(
    redis_conn: &mut redis::Connection,
    user_id: UserId,
) -> Result<Option<AdminUser>> {
    let admin_data: Option<String> = redis_conn
        .hget(key::ADMIN_PANEL_PERMISSIONS_KEY, user_id.0.to_string())
        .await?;
    
    match admin_data {
        Some(data) => {
            let admin_user: AdminUser = serde_json::from_str(&data)?;
            Ok(Some(admin_user))
        }
        None => Ok(None),
    }
}

/// Check if a user has a specific permission
pub async fn has_permission(
    redis_conn: &mut redis::Connection,
    user_id: UserId,
    permission: &AdminPermission,
) -> Result<bool> {
    // First check if user is an admin panel member
    if !is_admin_panel_member(redis_conn, user_id).await? {
        return Ok(false);
    }
    
    // Get admin user data
    if let Some(admin_user) = get_admin_user(redis_conn, user_id).await? {
        Ok(admin_user.has_permission(permission))
    } else {
        Ok(false)
    }
}

/// Check if a user has any of the given permissions
pub async fn has_any_permission(
    redis_conn: &mut redis::Connection,
    user_id: UserId,
    permissions: &[AdminPermission],
) -> Result<bool> {
    // First check if user is an admin panel member
    if !is_admin_panel_member(redis_conn, user_id).await? {
        return Ok(false);
    }
    
    // Get admin user data
    if let Some(admin_user) = get_admin_user(redis_conn, user_id).await? {
        Ok(admin_user.has_any_permission(permissions))
    } else {
        Ok(false)
    }
}

/// Add a user to the admin panel
pub async fn add_admin_user(
    redis_conn: &mut redis::Connection,
    user: &User,
    added_by: UserId,
    permission_group: Option<PermissionGroup>,
) -> Result<()> {
    // Check if user is already an admin
    if is_admin_panel_member(redis_conn, user.id).await? {
        return Err(anyhow::anyhow!("User is already an admin panel member"));
    }
    
    // Create new admin user
    let mut admin_user = AdminUser::new(
        user.id,
        user.username.clone(),
        user.full_name(),
        added_by,
    );
    
    // Add permissions based on permission group
    if let Some(group) = permission_group {
        for permission in group.permissions() {
            admin_user.add_permission(permission);
        }
    }
    
    // Add to admin panel members
    redis_conn.sadd(key::ADMIN_PANEL_MEMBERS_KEY, user.id.0.to_string()).await?;
    
    // Store admin user data
    let admin_data = serde_json::to_string(&admin_user)?;
    redis_conn
        .hset(key::ADMIN_PANEL_PERMISSIONS_KEY, user.id.0.to_string(), admin_data)
        .await?;
    
    Ok(())
}

/// Remove a user from the admin panel
pub async fn remove_admin_user(
    redis_conn: &mut redis::Connection,
    user_id: UserId,
) -> Result<()> {
    // Remove from admin panel members
    redis_conn.srem(key::ADMIN_PANEL_MEMBERS_KEY, user_id.0.to_string()).await?;
    
    // Remove admin user data
    redis_conn.hdel(key::ADMIN_PANEL_PERMISSIONS_KEY, user_id.0.to_string()).await?;
    
    Ok(())
}

/// Update admin user permissions
pub async fn update_admin_permissions(
    redis_conn: &mut redis::Connection,
    user_id: UserId,
    permissions: Vec<AdminPermission>,
) -> Result<()> {
    // Get existing admin user
    let mut admin_user = get_admin_user(redis_conn, user_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("User is not an admin panel member"))?;
    
    // Clear existing permissions and add new ones
    admin_user.permissions.clear();
    for permission in permissions {
        admin_user.add_permission(permission);
    }
    
    // Update last activity
    admin_user.update_activity();
    
    // Store updated admin user data
    let admin_data = serde_json::to_string(&admin_user)?;
    redis_conn
        .hset(key::ADMIN_PANEL_PERMISSIONS_KEY, user_id.0.to_string(), admin_data)
        .await?;
    
    Ok(())
}

/// Get all admin panel members
pub async fn get_all_admin_users(redis_conn: &mut redis::Connection) -> Result<Vec<AdminUser>> {
    let member_ids: Vec<String> = redis_conn.smembers(key::ADMIN_PANEL_MEMBERS_KEY).await?;
    let mut admin_users = Vec::new();
    
    for member_id_str in member_ids {
        if let Ok(user_id) = member_id_str.parse::<u64>() {
            if let Some(admin_user) = get_admin_user(redis_conn, UserId(user_id)).await? {
                admin_users.push(admin_user);
            }
        }
    }
    
    Ok(admin_users)
}

/// Check if user is admin in the current chat (for backward compatibility)
pub async fn is_user_admin_in_chat(bot: &Bot, chat: Chat, user_id: UserId) -> Result<bool> {
    if !chat.is_private() {
        let member = bot.get_chat_member(chat.id, user_id).await?;
        match member.status() {
            ChatMemberStatus::Owner => Ok(true),
            ChatMemberStatus::Administrator => Ok(true),
            _ => Ok(false),
        }
    } else {
        Ok(true)
    }
}

/// Enhanced admin check: combines admin panel membership with chat admin status
pub async fn is_admin_panel_admin(
    bot: &Bot,
    redis_conn: &mut redis::Connection,
    chat: Chat,
    user_id: UserId,
) -> Result<bool> {
    // First check if user is an admin panel member
    if !is_admin_panel_member(redis_conn, user_id).await? {
        return Ok(false);
    }
    
    // Then check if user is admin in the current chat
    is_user_admin_in_chat(bot, chat, user_id).await
}

/// Get admin panel status
pub async fn get_admin_panel_status(redis_conn: &mut redis::Connection) -> Result<AdminPanelStatus> {
    if !is_admin_panel_setup(redis_conn).await? {
        return Ok(AdminPanelStatus::NotSetup);
    }
    
    // For now, return Active if set up
    // TODO: Implement maintenance mode and disabled status
    Ok(AdminPanelStatus::Active)
}
