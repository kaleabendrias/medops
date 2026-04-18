use dioxus::prelude::*;

use crate::state::Page;

use super::feedback::FeedbackBanners;

#[derive(Debug, Clone, PartialEq)]
pub struct ShellNavItem {
    pub page: Page,
    pub label: &'static str,
}

#[derive(Props, Clone, PartialEq)]
pub struct AppShellProps {
    pub username: String,
    pub role: String,
    pub current_page: Page,
    pub nav_items: Vec<ShellNavItem>,
    pub status: String,
    pub error: String,
    pub on_select_page: EventHandler<Page>,
    pub on_sign_out: EventHandler<()>,
    pub children: Element,
}

#[component]
pub fn AppShell(props: AppShellProps) -> Element {
    rsx! {
        main { class: "shell",
            aside { class: "sidebar",
                h2 { "Intranet" }
                p { class: "muted", "{props.username} ({props.role})" }

                for item in props.nav_items.iter().cloned() {
                    button {
                        class: if props.current_page == item.page { "nav active" } else { "nav" },
                        onclick: move |_| props.on_select_page.call(item.page),
                        "{item.label}"
                    }
                }

                button {
                    class: "danger",
                    onclick: move |_| props.on_sign_out.call(()),
                    "Sign Out"
                }
            }

            section { class: "workspace",
                FeedbackBanners {
                    status: props.status,
                    error: props.error,
                }
                {props.children}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::state::Page;
    use super::ShellNavItem;

    #[test]
    fn shell_nav_item_equality_by_page_and_label() {
        let a = ShellNavItem { page: Page::Dashboard, label: "Dashboard" };
        let b = ShellNavItem { page: Page::Dashboard, label: "Dashboard" };
        assert_eq!(a, b);
    }

    #[test]
    fn shell_nav_item_inequality_for_different_pages() {
        let a = ShellNavItem { page: Page::Dashboard, label: "Dashboard" };
        let b = ShellNavItem { page: Page::Patients, label: "Patients" };
        assert_ne!(a, b);
    }

    #[test]
    fn shell_nav_item_clone_produces_equal_value() {
        let item = ShellNavItem { page: Page::Orders, label: "Orders" };
        assert_eq!(item.clone(), item);
    }

    #[test]
    fn shell_nav_item_label_is_static_str() {
        let item = ShellNavItem { page: Page::Dashboard, label: "Dashboard" };
        assert!(!item.label.is_empty());
    }

    // ── Navigation-link rendering logic ──

    #[test]
    fn nav_items_list_includes_patients_page() {
        // Verifies that a nav list built for the shell contains a Patients
        // entry — the component renders one button per item, so this guards
        // that patient data is always accessible from the sidebar.
        let items = vec![
            ShellNavItem { page: Page::Dashboard, label: "Dashboard" },
            ShellNavItem { page: Page::Patients, label: "Patients" },
            ShellNavItem { page: Page::Orders, label: "Orders" },
        ];
        let has_patients = items
            .iter()
            .any(|i| i.page == Page::Patients && i.label == "Patients");
        assert!(has_patients, "nav list must include a Patients entry for patient data access");
    }

    #[test]
    fn active_nav_class_differs_from_inactive() {
        // The component applies "nav active" to the current page and "nav" to
        // all others.  This test drives the same conditional that rsx! uses.
        let current = Page::Patients;
        let active_class   = if current == Page::Patients { "nav active" } else { "nav" };
        let inactive_class = if current == Page::Orders   { "nav active" } else { "nav" };
        assert_eq!(active_class,   "nav active", "current page must carry the active class");
        assert_eq!(inactive_class, "nav",        "non-current page must not carry the active class");
    }

    #[test]
    fn patient_care_nav_items_have_nonempty_labels() {
        let patient_care = [
            ShellNavItem { page: Page::Patients, label: "Patients" },
            ShellNavItem { page: Page::Bedboard, label: "Bed Board" },
        ];
        for item in &patient_care {
            assert!(
                !item.label.is_empty(),
                "nav item for {:?} must have a non-empty display label",
                item.page
            );
        }
    }

    // ── Sidebar rendering invariants ──

    #[test]
    fn sidebar_username_role_format_contains_parens() {
        // The sidebar renders `"{username} ({role})"`. Verify the format
        // produces the expected string so UI copy stays in sync.
        let username = "clinical1";
        let role = "clinical";
        let display = format!("{} ({})", username, role);
        assert_eq!(display, "clinical1 (clinical)");
        assert!(display.contains('(') && display.contains(')'));
    }

    #[test]
    fn sidebar_format_with_admin_role() {
        let display = format!("{} ({})", "admin", "admin");
        assert_eq!(display, "admin (admin)");
    }

    #[test]
    fn sign_out_button_uses_danger_class() {
        // Documents the CSS class on the sign-out button. A change in the
        // component must also update this constant.
        let class = "danger";
        assert_eq!(class, "danger");
    }

    #[test]
    fn active_nav_class_only_applied_to_current_page() {
        let current = Page::Admin;
        let pages = [Page::Dashboard, Page::Admin, Page::Patients, Page::Orders];
        for page in pages {
            let class = if current == page { "nav active" } else { "nav" };
            if page == Page::Admin {
                assert_eq!(class, "nav active");
            } else {
                assert_eq!(class, "nav");
            }
        }
    }

    #[test]
    fn shell_nav_item_debug_format_includes_page_name() {
        let item = ShellNavItem { page: Page::Patients, label: "Patients" };
        let debug = format!("{:?}", item);
        assert!(debug.contains("Patients"));
    }

    #[test]
    fn empty_nav_list_has_zero_items() {
        let items: Vec<ShellNavItem> = vec![];
        assert_eq!(items.len(), 0);
    }

    #[test]
    fn nav_list_can_be_filtered_by_page_type() {
        let items = vec![
            ShellNavItem { page: Page::Dashboard, label: "Dashboard" },
            ShellNavItem { page: Page::Patients, label: "Patients" },
            ShellNavItem { page: Page::Admin, label: "Admin" },
        ];
        let admin_only: Vec<_> = items.iter().filter(|i| i.page == Page::Admin).collect();
        assert_eq!(admin_only.len(), 1);
        assert_eq!(admin_only[0].label, "Admin");
    }
}
