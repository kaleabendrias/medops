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
                token: "token".to_string(),
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
}
