use crate::state::Page;

#[cfg(test)]
use crate::state::{can_access, SessionContext};

#[derive(Clone, Copy)]
pub struct NavItem {
    pub page: Page,
    pub label: &'static str,
}

pub fn nav_items() -> &'static [NavItem] {
    &[
        NavItem { page: Page::Dashboard, label: "Dashboard" },
        NavItem { page: Page::Patients, label: "Patients" },
        NavItem { page: Page::Bedboard, label: "Bed Board" },
        NavItem { page: Page::Dining, label: "Cafeteria" },
        NavItem { page: Page::Orders, label: "Orders" },
        NavItem { page: Page::Campaigns, label: "Campaigns" },
        NavItem { page: Page::Ingestion, label: "Ingestion" },
        NavItem { page: Page::Admin, label: "Admin" },
        NavItem { page: Page::Experiments, label: "Experiments" },
        NavItem { page: Page::Analytics, label: "Analytics" },
        NavItem { page: Page::Audits, label: "Audits" },
    ]
}

#[cfg(test)]
pub fn visible_nav_labels(session: &SessionContext) -> Vec<&'static str> {
    nav_items()
        .iter()
        .filter(|item| can_access(session, item.page))
        .map(|item| item.label)
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::state::{SessionContext, StoredSession};

    use super::{nav_items, visible_nav_labels};

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
    fn navigation_includes_all_expected_sections() {
        let labels = nav_items().iter().map(|x| x.label).collect::<Vec<_>>();
        assert!(labels.contains(&"Patients"));
        assert!(labels.contains(&"Bed Board"));
        assert!(labels.contains(&"Cafeteria"));
        assert!(labels.contains(&"Orders"));
        assert!(labels.contains(&"Campaigns"));
        assert!(labels.contains(&"Ingestion"));
        assert!(labels.contains(&"Admin"));
        assert!(labels.contains(&"Analytics"));
    }

    #[test]
    fn entitlement_filtered_navigation_hides_forbidden_pages() {
        let labels = visible_nav_labels(&ctx(&["dashboard", "dining", "orders", "campaigns"]));
        assert!(labels.contains(&"Cafeteria"));
        assert!(labels.contains(&"Orders"));
        assert!(!labels.contains(&"Patients"));
        assert!(!labels.contains(&"Admin"));
    }

    #[test]
    fn all_nav_labels_are_non_empty() {
        for item in nav_items() {
            assert!(!item.label.is_empty(), "nav item {:?} has empty label", item.page);
        }
    }

    #[test]
    fn nav_items_have_unique_pages() {
        let pages: Vec<_> = nav_items().iter().map(|x| x.page).collect();
        let mut seen = Vec::new();
        for p in &pages {
            assert!(!seen.contains(p), "duplicate page {:?} in nav_items", p);
            seen.push(*p);
        }
    }

    #[test]
    fn nav_items_have_unique_labels() {
        let labels: Vec<_> = nav_items().iter().map(|x| x.label).collect();
        let unique: std::collections::HashSet<_> = labels.iter().collect();
        assert_eq!(labels.len(), unique.len(), "duplicate labels in nav_items");
    }

    #[test]
    fn admin_entitlement_shows_all_admin_only_pages() {
        let labels = visible_nav_labels(&ctx(&["admin"]));
        assert!(labels.contains(&"Admin"));
        assert!(labels.contains(&"Experiments"));
        assert!(labels.contains(&"Analytics"));
        assert!(labels.contains(&"Audits"));
    }

    #[test]
    fn dashboard_entitlement_shows_only_dashboard() {
        let labels = visible_nav_labels(&ctx(&["dashboard"]));
        assert!(labels.contains(&"Dashboard"));
        assert!(!labels.contains(&"Patients"));
        assert!(!labels.contains(&"Admin"));
    }

    #[test]
    fn no_entitlements_shows_no_nav_items() {
        let labels = visible_nav_labels(&ctx(&[]));
        assert!(labels.is_empty(), "no entitlements should yield no visible nav items");
    }
}
