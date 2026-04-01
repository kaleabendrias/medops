use std::collections::HashSet;

use contracts::MenuEntitlementDto;
use serde::{Deserialize, Serialize};

#[cfg(target_arch = "wasm32")]
use gloo_storage::{LocalStorage, Storage};

#[cfg(target_arch = "wasm32")]
const SESSION_STORAGE_KEY: &str = "hospital_platform_session";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Dashboard,
    Patients,
    Bedboard,
    Dining,
    Orders,
    Campaigns,
    Ingestion,
    Admin,
    Experiments,
    Analytics,
    Audits,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct StoredSession {
    #[serde(default)]
    pub token: String,
    pub user_id: i64,
    pub username: String,
    pub role: String,
}

#[derive(Clone)]
pub struct SessionContext {
    pub stored: StoredSession,
    pub entitlements: HashSet<String>,
}

#[cfg(target_arch = "wasm32")]
pub fn save_session(session: &StoredSession) {
    let _ = LocalStorage::set(SESSION_STORAGE_KEY, session);
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_session(_session: &StoredSession) {}

#[cfg(target_arch = "wasm32")]
pub fn clear_session() {
    LocalStorage::delete(SESSION_STORAGE_KEY);
}

#[cfg(not(target_arch = "wasm32"))]
pub fn clear_session() {}

#[cfg(target_arch = "wasm32")]
pub fn load_session() -> Option<StoredSession> {
    LocalStorage::get(SESSION_STORAGE_KEY).ok()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_session() -> Option<StoredSession> {
    None
}

pub fn session_from_entitlements(
    stored: StoredSession,
    entitlements: Vec<MenuEntitlementDto>,
) -> SessionContext {
    SessionContext {
        stored,
        entitlements: entitlements
            .into_iter()
            .filter(|x| x.allowed)
            .map(|x| x.menu_key)
            .collect::<HashSet<_>>(),
    }
}

fn has_menu(session: &SessionContext, key: &str) -> bool {
    session.entitlements.contains(key)
}

pub fn can_access(session: &SessionContext, page: Page) -> bool {
    match page {
        Page::Dashboard => has_menu(session, "dashboard"),
        Page::Patients => has_menu(session, "patients") || has_menu(session, "clinical"),
        Page::Bedboard => has_menu(session, "bedboard"),
        Page::Dining => has_menu(session, "dining"),
        Page::Orders => has_menu(session, "orders"),
        Page::Campaigns => has_menu(session, "campaigns"),
        Page::Ingestion => has_menu(session, "ingestion") || has_menu(session, "admin"),
        Page::Admin => has_menu(session, "admin"),
        Page::Experiments => has_menu(session, "admin"),
        Page::Analytics => has_menu(session, "admin"),
        Page::Audits => has_menu(session, "audits") || has_menu(session, "admin"),
    }
}

pub fn page_to_hash(page: Page) -> &'static str {
    match page {
        Page::Dashboard => "#/dashboard",
        Page::Patients => "#/patients",
        Page::Bedboard => "#/bedboard",
        Page::Dining => "#/dining",
        Page::Orders => "#/orders",
        Page::Campaigns => "#/campaigns",
        Page::Ingestion => "#/ingestion",
        Page::Admin => "#/admin",
        Page::Experiments => "#/experiments",
        Page::Analytics => "#/analytics",
        Page::Audits => "#/audits",
    }
}

pub fn page_from_hash(hash: &str) -> Option<Page> {
    match hash.trim() {
        "#/dashboard" => Some(Page::Dashboard),
        "#/patients" => Some(Page::Patients),
        "#/bedboard" => Some(Page::Bedboard),
        "#/dining" => Some(Page::Dining),
        "#/orders" => Some(Page::Orders),
        "#/campaigns" => Some(Page::Campaigns),
        "#/ingestion" => Some(Page::Ingestion),
        "#/admin" => Some(Page::Admin),
        "#/experiments" => Some(Page::Experiments),
        "#/analytics" => Some(Page::Analytics),
        "#/audits" => Some(Page::Audits),
        _ => None,
    }
}

pub fn ensure_accessible_page(session: &SessionContext, requested: Page) -> Page {
    if can_access(session, requested) {
        return requested;
    }
    if can_access(session, Page::Dashboard) {
        return Page::Dashboard;
    }
    const ALL_PAGES: [Page; 11] = [
        Page::Dashboard,
        Page::Patients,
        Page::Bedboard,
        Page::Dining,
        Page::Orders,
        Page::Campaigns,
        Page::Ingestion,
        Page::Admin,
        Page::Experiments,
        Page::Analytics,
        Page::Audits,
    ];
    for page in ALL_PAGES {
        if can_access(session, page) {
            return page;
        }
    }
    Page::Dashboard
}

pub fn is_user_switch(previous: Option<&SessionContext>, next: &StoredSession) -> bool {
    match previous {
        Some(prev) => prev.stored.user_id != next.user_id,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{
        can_access, ensure_accessible_page, is_user_switch, page_from_hash, page_to_hash, Page,
        SessionContext, StoredSession,
    };

    fn ctx(keys: &[&str]) -> SessionContext {
        SessionContext {
            stored: StoredSession {
                token: "runtime_token".to_string(),
                user_id: 1,
                username: "tester".to_string(),
                role: "admin".to_string(),
            },
            entitlements: keys.iter().map(|k| (*k).to_string()).collect::<HashSet<_>>(),
        }
    }

    #[test]
    fn allows_dashboard_with_dashboard_entitlement() {
        let session = ctx(&["dashboard"]);
        assert!(can_access(&session, Page::Dashboard));
    }

    #[test]
    fn denies_admin_page_without_admin_entitlement() {
        let session = ctx(&["dashboard", "dining"]);
        assert!(!can_access(&session, Page::Admin));
    }

    #[test]
    fn allows_audits_with_audits_entitlement() {
        let session = ctx(&["audits"]);
        assert!(can_access(&session, Page::Audits));
    }

    #[test]
    fn allows_patients_when_clinical_entitled() {
        let session = ctx(&["clinical"]);
        assert!(can_access(&session, Page::Patients));
    }

    #[test]
    fn hash_roundtrip_dashboard() {
        assert_eq!(page_from_hash(page_to_hash(Page::Dashboard)), Some(Page::Dashboard));
    }

    #[test]
    fn inaccessible_hash_falls_back_to_dashboard() {
        let session = ctx(&["dashboard"]);
        let requested = page_from_hash("#/admin").unwrap_or(Page::Dashboard);
        assert_eq!(ensure_accessible_page(&session, requested), Page::Dashboard);
    }

    #[test]
    fn identifies_user_switch_on_new_user_id() {
        let previous = ctx(&["dashboard"]);
        let mut next = previous.stored.clone();
        next.user_id = 99;
        assert!(is_user_switch(Some(&previous), &next));
    }

    #[test]
    fn does_not_flag_first_login_as_user_switch() {
        let next = StoredSession {
            token: "token".to_string(),
            user_id: 1,
            username: "tester".to_string(),
            role: "admin".to_string(),
        };
        assert!(!is_user_switch(None, &next));
    }

    #[test]
    fn cafeteria_like_entitlements_cannot_access_patients() {
        let session = ctx(&["dashboard", "dining", "orders", "campaigns"]);
        assert!(!can_access(&session, Page::Patients));
        assert!(can_access(&session, Page::Dining));
    }

    #[test]
    fn inaccessible_page_falls_back_to_first_allowed_page() {
        let session = ctx(&["orders"]);
        let next = ensure_accessible_page(&session, Page::Admin);
        assert_eq!(next, Page::Orders);
    }
}
