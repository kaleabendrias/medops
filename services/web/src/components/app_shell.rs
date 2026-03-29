use dioxus::prelude::*;

use crate::state::Page;

use super::feedback::FeedbackBanners;

#[derive(Clone, PartialEq)]
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
