use tracing_subscriber::{fmt, EnvFilter};

const SENSITIVE_KEYS: &[&str] = &[
    "password",
    "token",
    "secret",
    "session_token",
    "password_hash",
    "authorization",
    "cookie",
];

pub fn init() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    fmt()
        .json()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .flatten_event(true)
        .init();
}

pub fn mask_sensitive_value(key: &str, value: &str) -> String {
    let lower = key.to_ascii_lowercase();
    if SENSITIVE_KEYS.iter().any(|k| lower.contains(k)) {
        "[REDACTED]".to_string()
    } else {
        value.to_string()
    }
}

pub fn sanitize_details(details: &serde_json::Value) -> serde_json::Value {
    match details {
        serde_json::Value::Object(map) => {
            let mut sanitized = serde_json::Map::new();
            for (k, v) in map {
                let lower = k.to_ascii_lowercase();
                if SENSITIVE_KEYS.iter().any(|s| lower.contains(s)) {
                    sanitized.insert(k.clone(), serde_json::Value::String("[REDACTED]".to_string()));
                } else {
                    sanitized.insert(k.clone(), sanitize_details(v));
                }
            }
            serde_json::Value::Object(sanitized)
        }
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_sensitive_redacts_password_fields() {
        assert_eq!(mask_sensitive_value("password", "secret123"), "[REDACTED]");
        assert_eq!(mask_sensitive_value("password_hash", "abc"), "[REDACTED]");
        assert_eq!(mask_sensitive_value("session_token", "tok"), "[REDACTED]");
    }

    #[test]
    fn mask_sensitive_passes_safe_fields() {
        assert_eq!(mask_sensitive_value("user_id", "42"), "42");
        assert_eq!(mask_sensitive_value("role", "admin"), "admin");
    }

    #[test]
    fn sanitize_details_redacts_nested_secrets() {
        let input = serde_json::json!({
            "user_id": 1,
            "password": "hunter2",
            "token": "abc123",
            "role": "admin"
        });
        let output = sanitize_details(&input);
        assert_eq!(output["password"], "[REDACTED]");
        assert_eq!(output["token"], "[REDACTED]");
        assert_eq!(output["user_id"], 1);
        assert_eq!(output["role"], "admin");
    }

    #[test]
    fn sanitize_details_handles_non_object() {
        let input = serde_json::json!("plain string");
        let output = sanitize_details(&input);
        assert_eq!(output, input);
    }
}
