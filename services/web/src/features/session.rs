pub fn can_reveal_revision_fields(role: &str) -> bool {
    matches!(role, "admin" | "doctor" | "nurse")
}

#[cfg(test)]
mod tests {
    use super::can_reveal_revision_fields;

    #[test]
    fn reveal_permission_is_role_bound() {
        assert!(can_reveal_revision_fields("admin"));
        assert!(can_reveal_revision_fields("doctor"));
        assert!(!can_reveal_revision_fields("member"));
    }
}
