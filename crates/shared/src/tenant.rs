//! Multi-tenant architecture — organizations, teams, users, RBAC.
//!
//! Data isolation per organization. All queries are scoped by org_id.

use serde::{Deserialize, Serialize};
// ─── Organization ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Organization {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub plan: Plan,
    pub settings: OrgSettings,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Plan {
    Free,
    Pro,
    Team,
    Enterprise,
}

impl Plan {
    pub fn max_seats(&self) -> u32 {
        match self {
            Plan::Free => 1,
            Plan::Pro => 5,
            Plan::Team => 25,
            Plan::Enterprise => 999,
        }
    }

    pub fn max_projects(&self) -> u32 {
        match self {
            Plan::Free => 3,
            Plan::Pro => 20,
            Plan::Team => 100,
            Plan::Enterprise => 999,
        }
    }

    pub fn monthly_token_budget(&self) -> u64 {
        match self {
            Plan::Free => 100_000,
            Plan::Pro => 10_000_000,
            Plan::Team => 100_000_000,
            Plan::Enterprise => 1_000_000_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgSettings {
    pub allowed_providers: Vec<String>,
    pub default_model: String,
    pub context_profile: String,
    pub auto_compression: bool,
}

impl Default for OrgSettings {
    fn default() -> Self {
        Self {
            allowed_providers: vec!["openai".to_string(), "anthropic".to_string()],
            default_model: "gpt-4o".to_string(),
            context_profile: "balanced".to_string(),
            auto_compression: true,
        }
    }
}

// ─── Team ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    pub id: String,
    pub org_id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
}

// ─── User ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub name: String,
    pub avatar_url: Option<String>,
    pub status: UserStatus,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum UserStatus {
    Active,
    Invited,
    Disabled,
}

// ─── Membership ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationMember {
    pub user_id: String,
    pub org_id: String,
    pub role: OrgRole,
    pub joined_at: String,
}

/// Organization-level role.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OrgRole {
    Owner,
    Admin,
    Developer,
    Reviewer,
    Viewer,
    Billing,
}

impl OrgRole {
    pub fn permissions(&self) -> Vec<&str> {
        match self {
            OrgRole::Owner => vec![
                "project:create", "project:delete", "project:read", "project:write",
                "session:run", "session:stop", "session:inject",
                "team:manage", "user:manage", "billing:manage",
                "admin:settings", "admin:audit",
            ],
            OrgRole::Admin => vec![
                "project:create", "project:read", "project:write",
                "session:run", "session:stop", "session:inject",
                "team:manage", "user:manage", "admin:settings",
            ],
            OrgRole::Developer => vec![
                "project:read", "project:write",
                "session:run", "session:stop", "session:inject",
            ],
            OrgRole::Reviewer => vec![
                "project:read",
                "session:read",
            ],
            OrgRole::Viewer => vec![
                "project:read",
                "session:read",
            ],
            OrgRole::Billing => vec![
                "billing:read", "billing:write",
            ],
        }
    }

    pub fn can(&self, permission: &str) -> bool {
        self.permissions().contains(&permission)
    }
}

// ─── RBAC ──────────────────────────────────────────────────────

/// Check if a user has permission in an organization.
pub fn check_permission(
    user_role: &OrgRole,
    required_permission: &str,
) -> bool {
    user_role.can(required_permission)
}

/// Check if a user has any of the required permissions.
pub fn check_any_permission(
    user_role: &OrgRole,
    permissions: &[&str],
) -> bool {
    permissions.iter().any(|p| user_role.can(p))
}

// ─── Data Isolation ───────────────────────────────────────────

/// Scope a query to an organization (for SQLite WHERE clause).
pub fn org_scope(org_id: &str) -> String {
    format!("WHERE org_id = '{}'", org_id)
}

// ─── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_limits() {
        assert_eq!(Plan::Free.max_seats(), 1);
        assert_eq!(Plan::Pro.max_seats(), 5);
        assert_eq!(Plan::Enterprise.max_projects(), 999);
    }

    #[test]
    fn test_org_role_permissions() {
        assert!(OrgRole::Owner.can("project:create"));
        assert!(OrgRole::Owner.can("billing:manage"));
        assert!(!OrgRole::Viewer.can("project:create"));
        assert!(!OrgRole::Developer.can("billing:manage"));
    }

    #[test]
    fn test_check_permission() {
        assert!(check_permission(&OrgRole::Developer, "session:run"));
        assert!(!check_permission(&OrgRole::Viewer, "session:run"));
    }

    #[test]
    fn test_check_any_permission() {
        assert!(check_any_permission(&OrgRole::Developer, &["session:run", "session:read"]));
        assert!(!check_any_permission(&OrgRole::Viewer, &["session:run", "session:inject"]));
    }
}