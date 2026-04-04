use dioxus::prelude::*;

use contracts::UserSummaryDto;

use crate::api;
use crate::state::SessionContext;

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
                    let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
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
                                let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
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
