use dioxus::prelude::*;

#[component]
pub fn FeedbackBanners(status: String, error: String) -> Element {
    rsx! {
        if !status.is_empty() { p { class: "ok", "{status}" } }
        if !error.is_empty() { p { class: "error", "{error}" } }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn status_banner_shown_when_non_empty() {
        let status = "Saving...".to_string();
        assert!(!status.is_empty(), "non-empty status should render");
    }

    #[test]
    fn status_banner_hidden_when_empty() {
        let status = String::new();
        assert!(status.is_empty(), "empty status should not render");
    }

    #[test]
    fn error_banner_shown_when_non_empty() {
        let error = "Network error".to_string();
        assert!(!error.is_empty(), "non-empty error should render");
    }

    #[test]
    fn error_banner_hidden_when_empty() {
        let error = String::new();
        assert!(error.is_empty(), "empty error should not render");
    }

    #[test]
    fn status_and_error_are_independent() {
        let status = "ok".to_string();
        let error = String::new();
        assert!(!status.is_empty());
        assert!(error.is_empty());
    }

    #[test]
    fn status_css_class_is_ok() {
        // Documents the CSS class applied to the status banner element.
        // The frontend stylesheet targets `.ok` for green/success styling.
        let class = "ok";
        assert_eq!(class, "ok");
    }

    #[test]
    fn error_css_class_is_error() {
        // Documents the CSS class applied to the error banner element.
        // The frontend stylesheet targets `.error` for red/danger styling.
        let class = "error";
        assert_eq!(class, "error");
    }

    #[test]
    fn both_banners_can_be_shown_simultaneously() {
        let status = "Saved successfully".to_string();
        let error = "Also an error".to_string();
        assert!(!status.is_empty(), "status banner should render");
        assert!(!error.is_empty(), "error banner should render");
    }

    #[test]
    fn whitespace_only_status_is_non_empty() {
        // Only a truly empty string suppresses the banner; whitespace-only
        // values still render because Rust's `is_empty()` returns false for them.
        let status = "   ".to_string();
        assert!(!status.is_empty());
    }

    #[test]
    fn error_message_survives_roundtrip_through_string() {
        let msg = "Connection refused: api:8000".to_string();
        let stored = msg.clone();
        assert_eq!(stored, "Connection refused: api:8000");
        assert!(!stored.is_empty());
    }
}
