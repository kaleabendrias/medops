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
                csrf_token: "token".to_string(),
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

    #[test]
    fn clinical_entitlement_grants_patients_page() {
        let decision = resolve_page_access(&ctx(&["dashboard", "clinical"]), Page::Patients);
        assert_eq!(decision.page, Page::Patients);
        assert!(!decision.forbidden);
    }

    #[test]
    fn admin_entitlement_grants_admin_page() {
        let decision = resolve_page_access(&ctx(&["dashboard", "admin"]), Page::Admin);
        assert_eq!(decision.page, Page::Admin);
        assert!(!decision.forbidden);
    }

    #[test]
    fn admin_entitlement_grants_experiments_page() {
        let decision = resolve_page_access(&ctx(&["admin"]), Page::Experiments);
        assert_eq!(decision.page, Page::Experiments);
        assert!(!decision.forbidden);
    }

    #[test]
    fn admin_entitlement_grants_analytics_page() {
        let decision = resolve_page_access(&ctx(&["admin"]), Page::Analytics);
        assert_eq!(decision.page, Page::Analytics);
        assert!(!decision.forbidden);
    }

    #[test]
    fn no_entitlements_redirects_to_dashboard_and_marks_forbidden() {
        // With zero entitlements there is no accessible page; resolve_page_access
        // falls back to Dashboard, but since dashboard entitlement is also absent,
        // forbidden=true correctly signals "nothing is accessible".
        let decision = resolve_page_access(&ctx(&[]), Page::Admin);
        assert_eq!(decision.page, Page::Dashboard);
        assert!(decision.forbidden, "no entitlements means all pages are forbidden");
    }

    #[test]
    fn ingestion_entitlement_grants_ingestion_page() {
        let decision = resolve_page_access(&ctx(&["ingestion"]), Page::Ingestion);
        assert_eq!(decision.page, Page::Ingestion);
        assert!(!decision.forbidden);
    }
}
