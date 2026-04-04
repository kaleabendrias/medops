use dioxus::prelude::*;

use contracts::AuditLogDto;

use crate::api;
use crate::state::SessionContext;

#[component]
pub fn AuditsPage(
    mut error: Signal<String>,
    session: Signal<Option<SessionContext>>,
    mut audits: Signal<Vec<AuditLogDto>>,
) -> Element {
    rsx! {
        article { class: "panel",
            h3 { "Audit Feed" }
            button {
                class: "primary",
                onclick: move |_| {
                    let token = session().as_ref().map(|s| s.stored.token.clone()).unwrap_or_default();
                    spawn(async move {
                        match api::list_audits(&token).await {
                            Ok(items) => audits.set(items),
                            Err(e) => error.set(e),
                        }
                    });
                },
                "Refresh Audits"
            }
            div { class: "cards",
                for a in audits() {
                    article { class: "card",
                        p { "{a.created_at} - {a.action_type}" }
                        p { class: "muted", "{a.entity_type} #{a.entity_id} by {a.actor_username}" }
                    }
                }
            }
        }
    }
}
