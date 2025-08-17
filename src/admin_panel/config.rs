//! Admin Panel Configuration and Redis Keys

/// **Admin Panel Redis Keys:** for storing admin panel data in Redis.
pub mod key {
    /// Key for storing the admin panel chat ID
    pub const ADMIN_PANEL_CHAT_KEY: &str = "admin:panel:chat_id";
    /// Key for storing admin panel members (SET)
    pub const ADMIN_PANEL_MEMBERS_KEY: &str = "admin:panel:members";
    /// Key for storing admin panel permissions (HASH: user_id -> permissions)
    pub const ADMIN_PANEL_PERMISSIONS_KEY: &str = "admin:panel:permissions";
    /// Key for storing admin panel audit log (LIST)
    pub const ADMIN_PANEL_AUDIT_LOG_KEY: &str = "admin:panel:audit_log";
    /// Key for storing admin panel settings (HASH)
    pub const ADMIN_PANEL_SETTINGS_KEY: &str = "admin:panel:settings";
    /// Key for storing monitored chats (SET)
    pub const ADMIN_PANEL_MONITORED_CHATS_KEY: &str = "admin:panel:monitored_chats";
}

/// **Admin Panel Settings:** default configuration values.
pub mod settings {
    /// Default maximum audit log entries to keep
    pub const MAX_AUDIT_LOG_ENTRIES: usize = 1000;
    /// Default admin panel session timeout (in seconds)
    pub const SESSION_TIMEOUT: u64 = 3600; // 1 hour
    /// Default rate limit for admin commands (commands per minute)
    pub const ADMIN_COMMAND_RATE_LIMIT: u32 = 30;
}

/// **Admin Panel Status:** possible states of the admin panel.
#[derive(Debug, Clone, PartialEq)]
pub enum AdminPanelStatus {
    /// Admin panel is not set up
    NotSetup,
    /// Admin panel is active and ready
    Active,
    /// Admin panel is in maintenance mode
    Maintenance,
    /// Admin panel is disabled
    Disabled,
}

impl std::fmt::Display for AdminPanelStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AdminPanelStatus::NotSetup => write!(f, "Not Setup"),
            AdminPanelStatus::Active => write!(f, "Active"),
            AdminPanelStatus::Maintenance => write!(f, "Maintenance"),
            AdminPanelStatus::Disabled => write!(f, "Disabled"),
        }
    }
}
