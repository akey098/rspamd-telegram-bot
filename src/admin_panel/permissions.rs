//! Admin Panel Permissions System

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use teloxide::types::UserId;

/// **Admin Permissions:** granular permissions for admin panel access.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AdminPermission {
    /// View statistics and dashboard
    ViewStats,
    /// Manage monitored chats
    ManageChats,
    /// Manage admin panel users
    ManageUsers,
    /// Configure bot settings
    ConfigureBot,
    /// View audit logs
    ViewAuditLog,
    /// Emergency control (stop/resume monitoring)
    EmergencyControl,
    /// Full access to all features
    FullAccess,
}

impl std::fmt::Display for AdminPermission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AdminPermission::ViewStats => write!(f, "View Statistics"),
            AdminPermission::ManageChats => write!(f, "Manage Chats"),
            AdminPermission::ManageUsers => write!(f, "Manage Users"),
            AdminPermission::ConfigureBot => write!(f, "Configure Bot"),
            AdminPermission::ViewAuditLog => write!(f, "View Audit Log"),
            AdminPermission::EmergencyControl => write!(f, "Emergency Control"),
            AdminPermission::FullAccess => write!(f, "Full Access"),
        }
    }
}

impl AdminPermission {
    /// Convert permission to string for storage
    pub fn to_string(&self) -> String {
        match self {
            AdminPermission::ViewStats => "view_stats".to_string(),
            AdminPermission::ManageChats => "manage_chats".to_string(),
            AdminPermission::ManageUsers => "manage_users".to_string(),
            AdminPermission::ConfigureBot => "configure_bot".to_string(),
            AdminPermission::ViewAuditLog => "view_audit_log".to_string(),
            AdminPermission::EmergencyControl => "emergency_control".to_string(),
            AdminPermission::FullAccess => "full_access".to_string(),
        }
    }

    /// Parse permission from string
    pub fn from_string(s: &str) -> Option<Self> {
        match s {
            "view_stats" => Some(AdminPermission::ViewStats),
            "manage_chats" => Some(AdminPermission::ManageChats),
            "manage_users" => Some(AdminPermission::ManageUsers),
            "configure_bot" => Some(AdminPermission::ConfigureBot),
            "view_audit_log" => Some(AdminPermission::ViewAuditLog),
            "emergency_control" => Some(AdminPermission::EmergencyControl),
            "full_access" => Some(AdminPermission::FullAccess),
            _ => None,
        }
    }

    /// Get all available permissions
    pub fn all_permissions() -> Vec<Self> {
        vec![
            AdminPermission::ViewStats,
            AdminPermission::ManageChats,
            AdminPermission::ManageUsers,
            AdminPermission::ConfigureBot,
            AdminPermission::ViewAuditLog,
            AdminPermission::EmergencyControl,
            AdminPermission::FullAccess,
        ]
    }
}

/// **Admin User:** represents an admin panel member with permissions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminUser {
    /// Telegram user ID
    pub user_id: UserId,
    /// Username (optional)
    pub username: Option<String>,
    /// Display name
    pub display_name: String,
    /// Set of permissions
    pub permissions: HashSet<AdminPermission>,
    /// Who added this admin
    pub added_by: UserId,
    /// When this admin was added
    pub added_at: chrono::DateTime<chrono::Utc>,
    /// Last activity timestamp
    pub last_activity: Option<chrono::DateTime<chrono::Utc>>,
}

impl AdminUser {
    /// Create a new admin user
    pub fn new(
        user_id: UserId,
        username: Option<String>,
        display_name: String,
        added_by: UserId,
    ) -> Self {
        Self {
            user_id,
            username,
            display_name,
            permissions: HashSet::new(),
            added_by,
            added_at: chrono::Utc::now(),
            last_activity: None,
        }
    }

    /// Check if user has a specific permission
    pub fn has_permission(&self, permission: &AdminPermission) -> bool {
        self.permissions.contains(&AdminPermission::FullAccess) || self.permissions.contains(permission)
    }

    /// Check if user has any of the given permissions
    pub fn has_any_permission(&self, permissions: &[AdminPermission]) -> bool {
        self.permissions.contains(&AdminPermission::FullAccess) 
            || permissions.iter().any(|p| self.permissions.contains(p))
    }

    /// Check if user has all of the given permissions
    pub fn has_all_permissions(&self, permissions: &[AdminPermission]) -> bool {
        self.permissions.contains(&AdminPermission::FullAccess) 
            || permissions.iter().all(|p| self.permissions.contains(p))
    }

    /// Add a permission to the user
    pub fn add_permission(&mut self, permission: AdminPermission) {
        self.permissions.insert(permission);
    }

    /// Remove a permission from the user
    pub fn remove_permission(&mut self, permission: &AdminPermission) {
        self.permissions.remove(permission);
    }

    /// Update last activity timestamp
    pub fn update_activity(&mut self) {
        self.last_activity = Some(chrono::Utc::now());
    }
}

/// **Permission Groups:** predefined permission sets for common roles.
#[derive(Debug, Clone)]
pub enum PermissionGroup {
    /// Read-only access to statistics and audit logs
    Viewer,
    /// Can manage chats and view statistics
    Moderator,
    /// Can manage users and configure basic settings
    Manager,
    /// Full administrative access
    Administrator,
}

impl PermissionGroup {
    /// Get permissions for a permission group
    pub fn permissions(&self) -> Vec<AdminPermission> {
        match self {
            PermissionGroup::Viewer => vec![
                AdminPermission::ViewStats,
                AdminPermission::ViewAuditLog,
            ],
            PermissionGroup::Moderator => vec![
                AdminPermission::ViewStats,
                AdminPermission::ManageChats,
                AdminPermission::ViewAuditLog,
            ],
            PermissionGroup::Manager => vec![
                AdminPermission::ViewStats,
                AdminPermission::ManageChats,
                AdminPermission::ManageUsers,
                AdminPermission::ConfigureBot,
                AdminPermission::ViewAuditLog,
            ],
            PermissionGroup::Administrator => vec![
                AdminPermission::FullAccess,
            ],
        }
    }

    /// Get display name for the permission group
    pub fn display_name(&self) -> &'static str {
        match self {
            PermissionGroup::Viewer => "Viewer",
            PermissionGroup::Moderator => "Moderator",
            PermissionGroup::Manager => "Manager",
            PermissionGroup::Administrator => "Administrator",
        }
    }
}
