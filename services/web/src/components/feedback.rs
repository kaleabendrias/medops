use dioxus::prelude::*;

#[component]
pub fn FeedbackBanners(status: String, error: String) -> Element {
    rsx! {
        if !status.is_empty() { p { class: "ok", "{status}" } }
        if !error.is_empty() { p { class: "error", "{error}" } }
    }
}
