use crate::state::{can_access, ensure_accessible_page, Page, SessionContext};

pub struct GuardDecision {
    pub page: Page,
    pub forbidden: bool,
}

pub fn resolve_page_access(session: &SessionContext, requested: Page) -> GuardDecision {
    let page = ensure_accessible_page(session, requested);
    GuardDecision {
        forbidden: !can_access(session, page),
        page,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::state::{Page, SessionContext, StoredSession};

    use super::resolve_page_access;

    fn ctx(keys: &[&str]) -> SessionContext {
        SessionContext {
            stored: StoredSession {
                token: "token".to_string(),
                user_id: 1,
                username: "tester".to_string(),
                role: "role".to_string(),
            },
            entitlements: keys.iter().map(|k| (*k).to_string()).collect::<HashSet<_>>(),
        }
    }

    #[test]
    fn deep_link_to_admin_is_guarded_for_non_admin() {
        let decision = resolve_page_access(&ctx(&["dashboard", "orders"]), Page::Admin);
        assert_eq!(decision.page, Page::Dashboard);
        assert!(!decision.forbidden);
    }

    #[test]
    fn allows_permitted_page_without_redirect() {
        let decision = resolve_page_access(&ctx(&["dashboard", "orders"]), Page::Orders);
        assert_eq!(decision.page, Page::Orders);
        assert!(!decision.forbidden);
    }
}
