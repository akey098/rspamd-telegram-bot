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
    /// View bot configuration
    ViewConfig,
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
            AdminPermission::ViewConfig => write!(f, "View Configuration"),
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
            AdminPermission::ViewConfig => "view_config".to_string(),
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
            "view_config" => Some(AdminPermission::ViewConfig),
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
            AdminPermission::ViewConfig,
            AdminPermission::EmergencyControl,
            AdminPermission::FullAccess,
        ]
    }

    /// Get permission description
    pub fn description(&self) -> &'static str {
        match self {
            AdminPermission::ViewStats => "View statistics, dashboard, and system health",
            AdminPermission::ManageChats => "Add/remove monitored chats and manage chat settings",
            AdminPermission::ManageUsers => "Add/remove admin panel members and manage permissions",
            AdminPermission::ConfigureBot => "Modify bot configuration and settings",
            AdminPermission::ViewAuditLog => "View audit logs and activity history",
            AdminPermission::ViewConfig => "View current bot configuration",
            AdminPermission::EmergencyControl => "Emergency stop/resume monitoring",
            AdminPermission::FullAccess => "Full access to all admin panel features",
        }
    }

    /// Check if permission is dangerous (requires confirmation)
    pub fn is_dangerous(&self) -> bool {
        matches!(self, 
            AdminPermission::EmergencyControl | 
            AdminPermission::FullAccess |
            AdminPermission::ManageUsers
        )
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
                AdminPermission::ViewConfig,
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

    /// Parse permission group from string
    pub fn from_string(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "viewer" => Some(PermissionGroup::Viewer),
            "moderator" => Some(PermissionGroup::Moderator),
            "manager" => Some(PermissionGroup::Manager),
            "administrator" | "admin" => Some(PermissionGroup::Administrator),
            _ => None,
        }
    }

    /// Get all available permission groups
    pub fn all_groups() -> Vec<Self> {
        vec![
            PermissionGroup::Viewer,
            PermissionGroup::Moderator,
            PermissionGroup::Manager,
            PermissionGroup::Administrator,
        ]
    }

    /// Get description for the permission group
    pub fn description(&self) -> &'static str {
        match self {
            PermissionGroup::Viewer => "Read-only access to view statistics and audit logs",
            PermissionGroup::Moderator => "Can manage chats and view statistics",
            PermissionGroup::Manager => "Can manage users and configure basic settings",
            PermissionGroup::Administrator => "Full administrative access to all features",
        }
    }
}

/// **Permission Template:** predefined permission configurations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionTemplate {
    pub name: String,
    pub description: String,
    pub permissions: Vec<AdminPermission>,
    pub is_dangerous: bool,
}

impl PermissionTemplate {
    /// Create a new permission template
    pub fn new(name: String, description: String, permissions: Vec<AdminPermission>) -> Self {
        let is_dangerous = permissions.iter().any(|p| p.is_dangerous());
        Self {
            name,
            description,
            permissions,
            is_dangerous,
        }
    }

    /// Get predefined templates
    pub fn predefined_templates() -> Vec<Self> {
        vec![
            PermissionTemplate::new(
                "Read Only".to_string(),
                "View-only access to statistics and logs".to_string(),
                vec![AdminPermission::ViewStats, AdminPermission::ViewAuditLog],
            ),
            PermissionTemplate::new(
                "Chat Manager".to_string(),
                "Manage chats and view statistics".to_string(),
                vec![
                    AdminPermission::ViewStats,
                    AdminPermission::ManageChats,
                    AdminPermission::ViewAuditLog,
                ],
            ),
            PermissionTemplate::new(
                "User Manager".to_string(),
                "Manage users and basic configuration".to_string(),
                vec![
                    AdminPermission::ViewStats,
                    AdminPermission::ManageUsers,
                    AdminPermission::ConfigureBot,
                    AdminPermission::ViewAuditLog,
                ],
            ),
            PermissionTemplate::new(
                "System Admin".to_string(),
                "Full system administration access".to_string(),
                vec![AdminPermission::FullAccess],
            ),
        ]
    }
}

/// **Permission Validator:** utilities for validating permission configurations
pub struct PermissionValidator;

impl PermissionValidator {
    /// Validate a set of permissions for conflicts
    pub fn validate_permissions(permissions: &[AdminPermission]) -> Result<(), String> {
        let mut permission_set = HashSet::new();
        
        for permission in permissions {
            if permission_set.contains(permission) {
                return Err(format!("Duplicate permission: {}", permission));
            }
            
            // Check for FullAccess conflicts
            if *permission == AdminPermission::FullAccess && permissions.len() > 1 {
                return Err("FullAccess permission should be used alone".to_string());
            }
            
            permission_set.insert(permission);
        }
        
        Ok(())
    }

    /// Check if permissions are compatible
    pub fn are_compatible(permissions: &[AdminPermission]) -> bool {
        Self::validate_permissions(permissions).is_ok()
    }

    /// Get minimum required permissions for a feature
    pub fn get_required_permissions(feature: &str) -> Vec<AdminPermission> {
        match feature {
            "dashboard" => vec![AdminPermission::ViewStats],
            "chat_management" => vec![AdminPermission::ManageChats],
            "user_management" => vec![AdminPermission::ManageUsers],
            "configuration" => vec![AdminPermission::ConfigureBot],
            "audit_log" => vec![AdminPermission::ViewAuditLog],
            "emergency_control" => vec![AdminPermission::EmergencyControl],
            _ => vec![],
        }
    }

    /// Check if user has required permissions for a feature
    pub fn has_feature_access(user_permissions: &[AdminPermission], feature: &str) -> bool {
        let required = Self::get_required_permissions(feature);
        required.is_empty() || required.iter().all(|p| user_permissions.contains(p))
    }
}

/// **Permission Export/Import:** utilities for permission configuration management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionConfig {
    pub user_id: UserId,
    pub username: Option<String>,
    pub display_name: String,
    pub permissions: Vec<AdminPermission>,
    pub added_by: UserId,
    pub added_at: chrono::DateTime<chrono::Utc>,
}

impl PermissionConfig {
    /// Export user permissions to configuration
    pub fn from_admin_user(admin_user: &AdminUser) -> Self {
        Self {
            user_id: admin_user.user_id,
            username: admin_user.username.clone(),
            display_name: admin_user.display_name.clone(),
            permissions: admin_user.permissions.iter().cloned().collect(),
            added_by: admin_user.added_by,
            added_at: admin_user.added_at,
        }
    }

    /// Convert to admin user
    pub fn to_admin_user(&self) -> AdminUser {
        let mut admin_user = AdminUser::new(
            self.user_id,
            self.username.clone(),
            self.display_name.clone(),
            self.added_by,
        );
        admin_user.added_at = self.added_at;
        for permission in &self.permissions {
            admin_user.add_permission(permission.clone());
        }
        admin_user
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

    /// Get permissions as sorted vector
    pub fn get_permissions_sorted(&self) -> Vec<AdminPermission> {
        let mut permissions: Vec<_> = self.permissions.iter().cloned().collect();
        permissions.sort_by(|a, b| a.to_string().cmp(&b.to_string()));
        permissions
    }

    /// Check if user has dangerous permissions
    pub fn has_dangerous_permissions(&self) -> bool {
        self.permissions.iter().any(|p| p.is_dangerous())
    }

    /// Get user's permission level (for display purposes)
    pub fn get_permission_level(&self) -> String {
        if self.has_permission(&AdminPermission::FullAccess) {
            "Administrator".to_string()
        } else if self.has_permission(&AdminPermission::ManageUsers) {
            "Manager".to_string()
        } else if self.has_permission(&AdminPermission::ManageChats) {
            "Moderator".to_string()
        } else {
            "Viewer".to_string()
        }
    }

    /// Export permissions to configuration
    pub fn export_config(&self) -> PermissionConfig {
        PermissionConfig::from_admin_user(self)
    }
}
