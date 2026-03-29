use dioxus::prelude::*;

use super::feedback::FeedbackBanners;

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
                p { class: "muted", "Seeded users: admin, clinical1, cafeteria1, member1" }
                FeedbackBanners {
                    status: props.status,
                    error: props.error,
                }
            }
        }
    }
}
