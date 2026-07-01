//! RBAC middleware — permission checking for API endpoints.

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};

use praxis_shared::tenant::OrgRole;

/// Permission check result.
#[derive(Debug)]
pub enum PermissionCheck {
    Allowed,
    Denied { reason: String },
}

/// Check if user has required permission.
pub fn check_permission(
    user_role: &OrgRole,
    required: &str,
) -> PermissionCheck {
    if user_role.can(required) {
        PermissionCheck::Allowed
    } else {
        PermissionCheck::Denied {
            reason: format!(
                "Role {:?} lacks permission: {}",
                user_role, required
            ),
        }
    }
}

/// RBAC middleware that checks permissions.
pub async fn rbac_middleware(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // TODO: Extract user from auth token
    // For now, allow all requests
    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    use super::*;
use praxis_shared::tenant::OrgRole;

    #[test]
    fn test_permission_check_allowed() {
        let result = check_permission(&OrgRole::Developer, "session:run");
        assert!(matches!(result, PermissionCheck::Allowed));
    }

    #[test]
    fn test_permission_check_denied() {
        let result = check_permission(&OrgRole::Viewer, "session:run");
        assert!(matches!(result, PermissionCheck::Denied { .. }));
    }
}