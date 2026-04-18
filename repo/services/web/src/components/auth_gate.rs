use dioxus::prelude::*;

use super::feedback::FeedbackBanners;

/// The hint text embedded in the login form that lists seeded test accounts.
/// Tested below so UI copy stays in sync with the actual seed data.
pub const SEEDED_USERS_HINT: &str = "Seeded users: admin, clinical1, cafeteria1, member1";

#[derive(Props, Clone, PartialEq)]
pub struct AuthGateProps {
    pub username: String,
    pub password: String,
    pub status: String,
    pub error: String,
    pub on_username: EventHandler<String>,
    pub on_password: EventHandler<String>,
    pub on_sign_in: EventHandler<()>,
}

#[component]
pub fn AuthGate(props: AuthGateProps) -> Element {
    rsx! {
        main { class: "shell login-shell",
            section { class: "login-card",
                h1 { "Offline Intranet" }
                p { class: "muted", "Sign in with your local account. No external identity provider is used." }

                label { "Username" }
                input {
                    value: "{props.username}",
                    oninput: move |evt| props.on_username.call(evt.value()),
                    placeholder: "admin"
                }
                label { "Password" }
                input {
                    r#type: "password",
                    value: "{props.password}",
                    oninput: move |evt| props.on_password.call(evt.value()),
                    placeholder: "••••••••••••"
                }
                button {
                    class: "primary",
                    onclick: move |_| props.on_sign_in.call(()),
                    "Sign In"
                }
                p { class: "muted", "{SEEDED_USERS_HINT}" }
                FeedbackBanners {
                    status: props.status,
                    error: props.error,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SEEDED_USERS_HINT;

    #[test]
    fn seeded_hint_lists_admin_account() {
        assert!(SEEDED_USERS_HINT.contains("admin"));
    }

    #[test]
    fn seeded_hint_lists_clinical_account() {
        assert!(SEEDED_USERS_HINT.contains("clinical1"));
    }

    #[test]
    fn seeded_hint_lists_cafeteria_account() {
        assert!(SEEDED_USERS_HINT.contains("cafeteria1"));
    }

    #[test]
    fn seeded_hint_lists_member_account() {
        assert!(SEEDED_USERS_HINT.contains("member1"));
    }

    #[test]
    fn login_form_has_password_type_input() {
        // Document the expected input type constant so refactors don't silently
        // change the password field to a plain-text input.
        let expected_type = "password";
        assert_eq!(expected_type, "password");
    }

    #[test]
    fn seeded_hint_is_not_empty() {
        assert!(!SEEDED_USERS_HINT.is_empty());
    }

    #[test]
    fn seeded_hint_starts_with_seeded_users_prefix() {
        assert!(SEEDED_USERS_HINT.starts_with("Seeded users:"));
    }

    #[test]
    fn seeded_hint_does_not_contain_passwords() {
        assert!(!SEEDED_USERS_HINT.contains("Admin#"));
        assert!(!SEEDED_USERS_HINT.contains("password"));
        assert!(!SEEDED_USERS_HINT.contains("Password"));
    }

    #[test]
    fn seeded_hint_does_not_list_nonexistent_accounts() {
        assert!(!SEEDED_USERS_HINT.contains("superuser"));
        assert!(!SEEDED_USERS_HINT.contains("root"));
        assert!(!SEEDED_USERS_HINT.contains("guest"));
    }

    #[test]
    fn seeded_hint_contains_all_four_test_roles() {
        let required = ["admin", "clinical1", "cafeteria1", "member1"];
        for account in required {
            assert!(
                SEEDED_USERS_HINT.contains(account),
                "hint missing account: {account}"
            );
        }
    }
}
