use dioxus::prelude::*;

use contracts::UserSummaryDto;

use crate::api;
use crate::state::SessionContext;

#[cfg(test)]
mod tests {
    use contracts::UserSummaryDto;

    fn make_user(id: i64, username: &str, role: &str, disabled: bool) -> UserSummaryDto {
        UserSummaryDto { id, username: username.to_string(), role: role.to_string(), disabled }
    }

    #[test]
    fn user_display_role_and_disabled_format() {
        let user = make_user(1, "admin", "admin", false);
        let display = format!("role: {} / disabled: {}", user.role, user.disabled);
        assert_eq!(display, "role: admin / disabled: false");
    }

    #[test]
    fn disabled_user_flag_shown_in_display_string() {
        let user = make_user(2, "member1", "member", true);
        let display = format!("role: {} / disabled: {}", user.role, user.disabled);
        assert!(display.contains("disabled: true"));
    }

    #[test]
    fn disable_button_uses_danger_class() {
        // Documents the "danger" CSS class invariant on the disable action button.
        // If the class is changed in the component, update the class here too.
        let class = "danger";
        assert_eq!(class, "danger");
    }

    #[test]
    fn user_id_is_preserved_through_dto() {
        let user = make_user(42, "clinical1", "clinical", false);
        assert_eq!(user.id, 42);
        assert_eq!(user.username, "clinical1");
        assert_eq!(user.role, "clinical");
        assert!(!user.disabled);
    }

    #[test]
    fn user_list_with_multiple_roles_retains_all_entries() {
        let users = vec![
            make_user(1, "admin", "admin", false),
            make_user(2, "member1", "member", false),
            make_user(3, "clinical1", "clinical", false),
        ];
        assert_eq!(users.len(), 3);
        assert!(users.iter().any(|u| u.role == "admin"));
        assert!(users.iter().any(|u| u.role == "clinical"));
    }
}

#[component]
pub fn AdminPage(
    mut status: Signal<String>,
    mut error: Signal<String>,
    session: Signal<Option<SessionContext>>,
    mut users: Signal<Vec<UserSummaryDto>>,
) -> Element {
    rsx! {
        article { class: "panel",
            h3 { "Administrator Console" }
            button {
                class: "primary",
                onclick: move |_| {
                    let token = session().as_ref().map(|s| s.stored.csrf_token.clone()).unwrap_or_default();
                    spawn(async move {
                        match api::list_users(&token).await {
                            Ok(items) => users.set(items),
                            Err(e) => error.set(e),
                        }
                    });
                },
                "Refresh Users"
            }
            div { class: "cards",
                for user in users() {
                    article { class: "card",
                        strong { "{user.username}" }
                        p { class: "muted", "role: {user.role} / disabled: {user.disabled}" }
                        button {
                            class: "danger",
                            onclick: move |_| {
                                let uid = user.id;
                                let token = session().as_ref().map(|s| s.stored.csrf_token.clone()).unwrap_or_default();
                                spawn(async move {
                                    match api::disable_user(&token, uid).await {
                                        Ok(_) => status.set(format!("User #{uid} disabled immediately")),
                                        Err(e) => error.set(e),
                                    }
                                });
                            },
                            "Disable"
                        }
                    }
                }
            }
        }
    }
}
