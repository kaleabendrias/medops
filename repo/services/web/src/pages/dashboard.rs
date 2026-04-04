use dioxus::prelude::*;

use crate::state::SessionContext;

#[component]
pub fn DashboardPage(ctx: SessionContext) -> Element {
    rsx! {
        article { class: "panel",
            h3 { "Role Entitlements" }
            p { class: "muted", "Navigation and page guards are driven from backend entitlements." }
            div { class: "chips",
                for key in ctx.entitlements.iter() { span { class: "chip", "{key}" } }
            }
        }
    }
}
