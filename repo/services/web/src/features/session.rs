use crate::state::SessionContext;

pub fn can_reveal_revision_fields(session: &SessionContext) -> bool {
    session.entitlements.contains("reveal_sensitive")
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::state::{SessionContext, StoredSession};

    use super::can_reveal_revision_fields;

    fn ctx(keys: &[&str]) -> SessionContext {
        SessionContext {
            stored: StoredSession {
                csrf_token: "t".to_string(),
                user_id: 1,
                username: "u".to_string(),
                role: "r".to_string(),
            },
            entitlements: keys.iter().map(|k| (*k).to_string()).collect::<HashSet<_>>(),
        }
    }

    #[test]
    fn reveal_permission_is_entitlement_driven() {
        assert!(can_reveal_revision_fields(&ctx(&["reveal_sensitive"])));
        assert!(!can_reveal_revision_fields(&ctx(&["dashboard", "clinical"])));
    }

    #[test]
    fn admin_with_entitlement_can_reveal() {
        assert!(can_reveal_revision_fields(&ctx(&["admin", "reveal_sensitive"])));
    }

    #[test]
    fn member_without_entitlement_cannot_reveal() {
        assert!(!can_reveal_revision_fields(&ctx(&["orders", "dashboard"])));
    }
}
