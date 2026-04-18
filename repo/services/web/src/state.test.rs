use std::collections::HashSet;

use contracts::MenuEntitlementDto;

use crate::state::{
    can_access, ensure_accessible_page, is_user_switch, page_from_hash, page_to_hash,
    session_from_entitlements, Page, SessionContext, StoredSession,
};

fn make_session(keys: &[&str]) -> SessionContext {
    SessionContext {
        stored: StoredSession {
            csrf_token: "tok".to_string(),
            user_id: 42,
            username: "ext_tester".to_string(),
            role: "admin".to_string(),
        },
        entitlements: keys.iter().map(|k| (*k).to_string()).collect::<HashSet<_>>(),
    }
}

fn entitlement(key: &str, allowed: bool) -> MenuEntitlementDto {
    MenuEntitlementDto { menu_key: key.to_string(), allowed }
}

#[test]
fn all_pages_have_hash_roundtrip() {
    use Page::*;
    for page in [
        Dashboard, Patients, Bedboard, Dining, Orders, Campaigns, Ingestion, Admin, Experiments,
        Analytics, Audits,
    ] {
        assert_eq!(page_from_hash(page_to_hash(page)), Some(page));
    }
}

#[test]
fn session_from_entitlements_filters_disallowed() {
    let stored = StoredSession {
        csrf_token: "t".to_string(),
        user_id: 1,
        username: "u".to_string(),
        role: "r".to_string(),
    };
    let list = vec![
        entitlement("dashboard", true),
        entitlement("admin", false),
        entitlement("dining", true),
    ];
    let ctx = session_from_entitlements(stored, list);
    assert!(ctx.entitlements.contains("dashboard"));
    assert!(!ctx.entitlements.contains("admin"));
    assert!(ctx.entitlements.contains("dining"));
}

#[test]
fn experiments_page_requires_admin_entitlement() {
    assert!(can_access(&make_session(&["admin"]), Page::Experiments));
    assert!(!can_access(&make_session(&["orders", "dining"]), Page::Experiments));
}

#[test]
fn analytics_page_requires_admin_entitlement() {
    assert!(can_access(&make_session(&["admin"]), Page::Analytics));
    assert!(!can_access(&make_session(&["orders"]), Page::Analytics));
}

#[test]
fn ingestion_accessible_with_ingestion_or_admin() {
    assert!(can_access(&make_session(&["ingestion"]), Page::Ingestion));
    assert!(can_access(&make_session(&["admin"]), Page::Ingestion));
    assert!(!can_access(&make_session(&["orders"]), Page::Ingestion));
}

#[test]
fn ensure_accessible_page_with_no_entitlements_falls_back_to_dashboard() {
    let ctx = make_session(&[]);
    assert_eq!(ensure_accessible_page(&ctx, Page::Patients), Page::Dashboard);
}

#[test]
fn unknown_hash_returns_none() {
    assert_eq!(page_from_hash("#/unknown-page"), None);
    assert_eq!(page_from_hash(""), None);
}

#[test]
fn same_user_login_is_not_user_switch() {
    let ctx = make_session(&["dashboard"]);
    let same_user = ctx.stored.clone();
    assert!(!is_user_switch(Some(&ctx), &same_user));
}

#[test]
fn bedboard_page_requires_bedboard_entitlement() {
    assert!(can_access(&make_session(&["bedboard"]), Page::Bedboard));
    assert!(!can_access(&make_session(&["orders"]), Page::Bedboard));
}

#[test]
fn audits_accessible_with_admin_or_audits_entitlement() {
    assert!(can_access(&make_session(&["admin"]), Page::Audits));
    assert!(can_access(&make_session(&["audits"]), Page::Audits));
    assert!(!can_access(&make_session(&["orders"]), Page::Audits));
}

#[test]
fn campaigns_page_requires_campaigns_entitlement() {
    assert!(can_access(&make_session(&["campaigns"]), Page::Campaigns));
    assert!(!can_access(&make_session(&["orders"]), Page::Campaigns));
}

#[test]
fn ensure_accessible_falls_back_to_first_allowed_non_dashboard() {
    let ctx = make_session(&["orders"]);
    assert_eq!(ensure_accessible_page(&ctx, Page::Admin), Page::Orders);
}
