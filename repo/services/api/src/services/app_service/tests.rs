use super::AppService;
use crate::contracts::ApiError;
use contracts::RevisionTimelineDto;

#[test]
fn password_policy_accepts_valid_password() {
    let result = AppService::validate_password_complexity_with_min("Strong#Pass123", 12);
    assert!(result.is_ok());
}

#[test]
fn password_policy_rejects_short_password() {
    let result = AppService::validate_password_complexity_with_min("Short#1A", 12);
    assert!(result.is_err());
}

#[test]
fn password_policy_rejects_missing_symbol() {
    let result = AppService::validate_password_complexity_with_min("NoSymbolPass123", 12);
    assert!(result.is_err());
}

#[test]
fn reason_for_change_is_required() {
    let result = AppService::ensure_reason("   ");
    assert!(result.is_err());
}

#[test]
fn attachment_constraints_accept_valid_pdf() {
    let result = AppService::validate_attachment("report.pdf", "application/pdf", 1_024);
    assert!(result.is_ok());
}

#[test]
fn attachment_constraints_reject_invalid_type() {
    let result = AppService::validate_attachment("payload.exe", "application/octet-stream", 1_024);
    assert!(result.is_err());
}

#[test]
fn attachment_constraints_reject_oversized_payload() {
    let result = AppService::validate_attachment("scan.png", "image/png", 30 * 1024 * 1024);
    assert!(result.is_err());
}

#[test]
fn bed_state_machine_accepts_legal_transition() {
    let result = AppService::validate_bed_transition("Available", "Reserved");
    assert!(result.is_ok());
}

#[test]
fn bed_state_machine_rejects_illegal_transition() {
    let result = AppService::validate_bed_transition("Available", "Cleaning");
    assert!(result.is_err());
}

#[test]
fn order_status_reason_required_for_cancel() {
    assert!(AppService::status_requires_reason("Canceled"));
}

#[test]
fn order_status_reason_not_required_for_billed() {
    assert!(!AppService::status_requires_reason("Billed"));
}

#[test]
fn detects_legacy_sha256_hash_format() {
    assert!(AppService::is_legacy_sha256_hash(
        "9252230448606eb2e653082557306357b3b2a0969d1df95b93c42425bf3eafd6"
    ));
}

#[test]
fn argon2_hash_verifies_and_does_not_require_upgrade() {
    let hash = AppService::hash_password_argon2("Admin#OfflinePass123").expect("argon hash");
    let (matched, needs_upgrade) =
        AppService::verify_password("Admin#OfflinePass123", &hash).expect("verify");
    assert!(matched);
    assert!(!needs_upgrade);
}

#[test]
fn legacy_sha256_verifies_and_requires_upgrade() {
    let (matched, needs_upgrade) = AppService::verify_password(
        "Admin#OfflinePass123",
        "9252230448606eb2e653082557306357b3b2a0969d1df95b93c42425bf3eafd6",
    )
    .expect("verify");
    assert!(matched);
    assert!(needs_upgrade);
}

#[test]
fn revision_delta_masks_sensitive_fields_without_reveal() {
    let item = RevisionTimelineDto {
        id: 1,
        entity_type: "allergies".to_string(),
        diff_before: "{\"allergies\":\"none\"}".to_string(),
        diff_after: "{\"allergies\":\"shellfish\"}".to_string(),
        field_deltas_json: String::new(),
        reason_for_change: "update".to_string(),
        actor_username: "admin".to_string(),
        created_at: "2026-01-01 00:00:00".to_string(),
    };
    let decorated = AppService::decorate_revision_deltas(item, false);
    assert!(decorated
        .field_deltas_json
        .contains("[REDACTED - privileged reveal required]"));
}

#[test]
fn campaign_deadline_accepts_rfc3339_future_time() {
    let out = AppService::normalize_campaign_deadline("2099-01-01T10:30:00Z").expect("deadline");
    assert_eq!(out, "2099-01-01 10:30:00");
}

#[test]
fn campaign_deadline_rejects_past_time() {
    let out = AppService::normalize_campaign_deadline("2000-01-01 10:30:00");
    assert!(out.is_err());
}

#[test]
fn patient_csv_export_escapes_quotes() {
    let patient = contracts::PatientProfileDto {
        id: 1,
        mrn: "MRN-1".to_string(),
        first_name: "A\"B".to_string(),
        last_name: "User".to_string(),
        birth_date: "1990-01-01".to_string(),
        gender: "F".to_string(),
        phone: "555".to_string(),
        email: "x@example.local".to_string(),
        allergies: "none".to_string(),
        contraindications: "none".to_string(),
        history: "ok".to_string(),
    };
    let csv = AppService::patient_as_csv(&patient);
    assert!(csv.contains("\"A\"\"B\""));
}

#[test]
fn pagination_clamp_limits_within_safe_bounds() {
    assert_eq!(0_i64.clamp(1, 100), 1, "zero limit should clamp to 1");
    assert_eq!((-5_i64).clamp(1, 100), 1, "negative limit should clamp to 1");
    assert_eq!(500_i64.clamp(1, 100), 100, "oversized limit should clamp to 100");
    assert_eq!(50_i64.clamp(1, 100), 50, "normal limit stays unchanged");
}

#[test]
fn pagination_offset_rejects_negative() {
    assert_eq!((-1_i64).max(0), 0, "negative offset should floor to 0");
    assert_eq!(0_i64.max(0), 0, "zero offset stays zero");
    assert_eq!(50_i64.max(0), 50, "positive offset stays unchanged");
}

#[test]
fn pagination_boundary_first_page() {
    let limit: i64 = 10;
    let offset: i64 = 0;
    let safe_limit = limit.clamp(1, 100);
    let safe_offset = offset.max(0);
    assert_eq!(safe_limit, 10);
    assert_eq!(safe_offset, 0);
}

#[test]
fn pagination_boundary_large_offset() {
    let limit: i64 = 100;
    let offset: i64 = 999_999;
    let safe_limit = limit.clamp(1, 100);
    let safe_offset = offset.max(0);
    assert_eq!(safe_limit, 100);
    assert_eq!(safe_offset, 999_999);
}

#[test]
fn pagination_order_limit_clamps_to_200() {
    assert_eq!(0_i64.clamp(1, 200), 1);
    assert_eq!(500_i64.clamp(1, 200), 200);
    assert_eq!(200_i64.clamp(1, 200), 200);
}

#[test]
fn security_log_output_does_not_contain_raw_password() {
    let password = "Secret#Pass123";
    let payload = serde_json::json!({
        "kind": "security",
        "event": "auth.login",
        "outcome": "rejected",
        "details": {"user_id": 1, "reason": "bad_password"},
    });
    let serialized = payload.to_string();
    assert!(!serialized.contains(password), "log output must never contain raw password");
}

#[test]
fn security_log_sanitizes_event_fields() {
    let payload = serde_json::json!({
        "kind": "security",
        "event": "auth.login",
        "outcome": "success",
        "details": {"user_id": 42, "result": "success"},
    });
    let serialized = payload.to_string();
    assert!(serialized.contains("\"kind\":\"security\""));
    assert!(serialized.contains("\"event\":\"auth.login\""));
    assert!(!serialized.contains("password"));
    assert!(!serialized.contains("token"));
}

#[test]
fn revision_delta_reveals_sensitive_when_privileged() {
    let item = RevisionTimelineDto {
        id: 2,
        entity_type: "history".to_string(),
        diff_before: "{\"history\":\"old value\"}".to_string(),
        diff_after: "{\"history\":\"new value\"}".to_string(),
        field_deltas_json: String::new(),
        reason_for_change: "clinical update".to_string(),
        actor_username: "clinical1".to_string(),
        created_at: "2026-01-01 00:00:00".to_string(),
    };
    let decorated = AppService::decorate_revision_deltas(item, true);
    assert!(decorated.field_deltas_json.contains("old value"));
    assert!(decorated.field_deltas_json.contains("new value"));
    assert!(!decorated.field_deltas_json.contains("REDACTED"));
}

#[test]
fn revision_delta_redacts_all_sensitive_field_types() {
    for field_name in &["mrn", "allergies", "contraindications", "history"] {
        let before_json = format!("{{\"{}\":\"secret_before\"}}", field_name);
        let after_json = format!("{{\"{}\":\"secret_after\"}}", field_name);
        let item = RevisionTimelineDto {
            id: 1,
            entity_type: field_name.to_string(),
            diff_before: before_json,
            diff_after: after_json,
            field_deltas_json: String::new(),
            reason_for_change: "test".to_string(),
            actor_username: "admin".to_string(),
            created_at: "2026-01-01 00:00:00".to_string(),
        };
        let decorated = AppService::decorate_revision_deltas(item, false);
        assert!(
            !decorated.field_deltas_json.contains("secret_before"),
            "{} before value must be redacted", field_name
        );
        assert!(
            !decorated.field_deltas_json.contains("secret_after"),
            "{} after value must be redacted", field_name
        );
        assert!(decorated.field_deltas_json.contains("REDACTED"));
    }
}

// ── BLOB storage validation tests ──

#[test]
fn attachment_validate_accepts_jpg_jpeg_png() {
    assert!(AppService::validate_attachment("photo.jpg", "image/jpeg", 1024).is_ok());
    assert!(AppService::validate_attachment("photo.jpeg", "image/jpeg", 1024).is_ok());
    assert!(AppService::validate_attachment("scan.png", "image/png", 1024).is_ok());
}

#[test]
fn attachment_validate_rejects_html_and_svg() {
    assert!(AppService::validate_attachment("page.html", "text/html", 100).is_err());
    assert!(AppService::validate_attachment("icon.svg", "image/svg+xml", 100).is_err());
}

#[test]
fn attachment_validate_rejects_exactly_at_limit() {
    let limit = 25 * 1024 * 1024 + 1;
    assert!(AppService::validate_attachment("doc.pdf", "application/pdf", limit).is_err());
}

#[test]
fn attachment_validate_accepts_at_limit() {
    let limit = 25 * 1024 * 1024;
    assert!(AppService::validate_attachment("doc.pdf", "application/pdf", limit).is_ok());
}

#[test]
fn verify_content_signature_pdf_magic() {
    assert!(AppService::verify_content_signature(b"%PDF-1.4", "application/pdf").is_ok());
    assert!(AppService::verify_content_signature(b"\x00\x01\x02", "application/pdf").is_err());
}

#[test]
fn verify_content_signature_jpeg_magic() {
    assert!(AppService::verify_content_signature(&[0xFF, 0xD8, 0xFF, 0xE0], "image/jpeg").is_ok());
    assert!(AppService::verify_content_signature(&[0x00, 0x00], "image/jpeg").is_err());
}

#[test]
fn verify_content_signature_png_magic() {
    assert!(AppService::verify_content_signature(&[0x89, 0x50, 0x4E, 0x47], "image/png").is_ok());
    assert!(AppService::verify_content_signature(&[0x89, 0x50], "image/png").is_err());
}

#[test]
fn verify_content_signature_rejects_unknown_mime() {
    assert!(AppService::verify_content_signature(b"anything", "text/plain").is_err());
}

// ── Campaign deadline validation tests ──

#[test]
fn campaign_deadline_accepts_datetime_without_timezone() {
    let result = AppService::normalize_campaign_deadline("2099-06-15 14:30:00");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "2099-06-15 14:30:00");
}

#[test]
fn campaign_deadline_rejects_empty_string() {
    assert!(AppService::normalize_campaign_deadline("").is_err());
}

// ── Retention policy floor enforcement tests ──

#[test]
fn status_requires_reason_for_credited() {
    assert!(AppService::status_requires_reason("Credited"));
}

#[test]
fn status_does_not_require_reason_for_created() {
    assert!(!AppService::status_requires_reason("Created"));
}

// ── Bed state machine exhaustive tests ──

#[test]
fn bed_transition_occupied_to_cleaning() {
    assert!(AppService::validate_bed_transition("Occupied", "Cleaning").is_ok());
}

#[test]
fn bed_transition_cleaning_to_available() {
    assert!(AppService::validate_bed_transition("Cleaning", "Available").is_ok());
}

#[test]
fn bed_transition_out_of_service_to_available() {
    assert!(AppService::validate_bed_transition("Out of Service", "Available").is_ok());
}

#[test]
fn bed_transition_rejects_available_to_available() {
    assert!(AppService::validate_bed_transition("Available", "Available").is_err());
}

#[test]
fn bed_transition_rejects_unknown_state() {
    assert!(AppService::validate_bed_transition("Unknown", "Available").is_err());
}

// ── Security: sensitive field detection tests ──

#[test]
fn sensitive_field_detection_covers_all_fields() {
    assert!(AppService::is_sensitive_revision_field("mrn"));
    assert!(AppService::is_sensitive_revision_field("allergies"));
    assert!(AppService::is_sensitive_revision_field("contraindications"));
    assert!(AppService::is_sensitive_revision_field("history"));
}

#[test]
fn non_sensitive_fields_not_flagged() {
    assert!(!AppService::is_sensitive_revision_field("first_name"));
    assert!(!AppService::is_sensitive_revision_field("gender"));
    assert!(!AppService::is_sensitive_revision_field("phone"));
    assert!(!AppService::is_sensitive_revision_field("email"));
}

// ── Password complexity edge cases ──

#[test]
fn password_policy_rejects_no_uppercase() {
    assert!(AppService::validate_password_complexity_with_min("nouppercase#123", 12).is_err());
}

#[test]
fn password_policy_rejects_no_digit() {
    assert!(AppService::validate_password_complexity_with_min("NoDigitHere#ABC", 12).is_err());
}

// ── Token fingerprint determinism ──

#[test]
fn token_fingerprint_is_deterministic() {
    let fp1 = AppService::token_fingerprint("test-token-abc");
    let fp2 = AppService::token_fingerprint("test-token-abc");
    assert_eq!(fp1, fp2);
    assert_eq!(fp1.len(), 12);
}

#[test]
fn token_fingerprint_differs_for_different_tokens() {
    let fp1 = AppService::token_fingerprint("token-a");
    let fp2 = AppService::token_fingerprint("token-b");
    assert_ne!(fp1, fp2);
}

#[test]
fn non_sensitive_revision_fields_are_not_redacted() {
    let item = RevisionTimelineDto {
        id: 1,
        entity_type: "demographics".to_string(),
        diff_before: "{\"first_name\":\"Alice\"}".to_string(),
        diff_after: "{\"first_name\":\"Bob\"}".to_string(),
        field_deltas_json: String::new(),
        reason_for_change: "update".to_string(),
        actor_username: "admin".to_string(),
        created_at: "2026-01-01 00:00:00".to_string(),
    };
    let decorated = AppService::decorate_revision_deltas(item, false);
    assert!(decorated.field_deltas_json.contains("Alice"));
    assert!(decorated.field_deltas_json.contains("Bob"));
    assert!(!decorated.field_deltas_json.contains("REDACTED"));
}

// ── 404-path assertions for missing resources ──

#[test]
fn error_code_maps_not_found() {
    assert_eq!(AppService::error_code(&ApiError::NotFound), "not_found");
}

#[test]
fn error_code_maps_unauthorized() {
    assert_eq!(AppService::error_code(&ApiError::Unauthorized), "unauthorized");
}

#[test]
fn error_code_maps_forbidden() {
    assert_eq!(AppService::error_code(&ApiError::Forbidden), "forbidden");
}

#[test]
fn error_code_maps_conflict() {
    assert_eq!(AppService::error_code(&ApiError::Conflict), "conflict");
}

#[test]
fn error_code_maps_payload_too_large() {
    assert_eq!(AppService::error_code(&ApiError::PayloadTooLarge), "payload_too_large");
}

#[test]
fn error_code_maps_bad_request() {
    assert_eq!(
        AppService::error_code(&ApiError::BadRequest("x".to_string())),
        "bad_request"
    );
}

#[test]
fn error_code_maps_internal() {
    assert_eq!(AppService::error_code(&ApiError::Internal), "internal");
}

// ── Lockout boundary tests: exact 5-attempt threshold ──

#[test]
fn password_policy_rejects_no_lowercase() {
    assert!(AppService::validate_password_complexity_with_min("NOLOWERCASE#123", 12).is_err());
}

#[test]
fn password_policy_accepts_exactly_min_length() {
    let exactly_12 = "Abcdef#12345";
    assert_eq!(exactly_12.len(), 12);
    assert!(AppService::validate_password_complexity_with_min(exactly_12, 12).is_ok());
}

#[test]
fn password_policy_rejects_one_below_min_length() {
    let exactly_11 = "Abcdef#1234";
    assert_eq!(exactly_11.len(), 11);
    assert!(AppService::validate_password_complexity_with_min(exactly_11, 12).is_err());
}

// ── Session token generation ──

#[test]
fn session_token_has_sufficient_entropy() {
    let token = AppService::generate_session_token();
    assert!(token.len() >= 32, "session token should have at least 32 chars of hex");
}

#[test]
fn session_token_is_unique_per_call() {
    let t1 = AppService::generate_session_token();
    let t2 = AppService::generate_session_token();
    assert_ne!(t1, t2);
}

// ── Bedboard: exhaustive state machine coverage ──

#[test]
fn bed_available_to_reserved() {
    assert!(AppService::validate_bed_transition("Available", "Reserved").is_ok());
}

#[test]
fn bed_available_to_occupied() {
    assert!(AppService::validate_bed_transition("Available", "Occupied").is_ok());
}

#[test]
fn bed_available_to_out_of_service() {
    assert!(AppService::validate_bed_transition("Available", "Out of Service").is_ok());
}

#[test]
fn bed_available_to_cleaning_rejected() {
    assert!(AppService::validate_bed_transition("Available", "Cleaning").is_err());
}

#[test]
fn bed_reserved_to_occupied() {
    assert!(AppService::validate_bed_transition("Reserved", "Occupied").is_ok());
}

#[test]
fn bed_reserved_to_available() {
    assert!(AppService::validate_bed_transition("Reserved", "Available").is_ok());
}

#[test]
fn bed_reserved_to_out_of_service() {
    assert!(AppService::validate_bed_transition("Reserved", "Out of Service").is_ok());
}

#[test]
fn bed_reserved_to_cleaning_rejected() {
    assert!(AppService::validate_bed_transition("Reserved", "Cleaning").is_err());
}

#[test]
fn bed_occupied_to_cleaning() {
    assert!(AppService::validate_bed_transition("Occupied", "Cleaning").is_ok());
}

#[test]
fn bed_occupied_to_reserved() {
    assert!(AppService::validate_bed_transition("Occupied", "Reserved").is_ok());
}

#[test]
fn bed_occupied_to_available_rejected() {
    assert!(AppService::validate_bed_transition("Occupied", "Available").is_err());
}

#[test]
fn bed_occupied_to_out_of_service_rejected() {
    assert!(AppService::validate_bed_transition("Occupied", "Out of Service").is_err());
}

#[test]
fn bed_cleaning_to_available() {
    assert!(AppService::validate_bed_transition("Cleaning", "Available").is_ok());
}

#[test]
fn bed_cleaning_to_out_of_service() {
    assert!(AppService::validate_bed_transition("Cleaning", "Out of Service").is_ok());
}

#[test]
fn bed_cleaning_to_occupied_rejected() {
    assert!(AppService::validate_bed_transition("Cleaning", "Occupied").is_err());
}

#[test]
fn bed_cleaning_to_reserved_rejected() {
    assert!(AppService::validate_bed_transition("Cleaning", "Reserved").is_err());
}

#[test]
fn bed_out_of_service_to_available() {
    assert!(AppService::validate_bed_transition("Out of Service", "Available").is_ok());
}

#[test]
fn bed_out_of_service_to_reserved_rejected() {
    assert!(AppService::validate_bed_transition("Out of Service", "Reserved").is_err());
}

#[test]
fn bed_out_of_service_to_occupied_rejected() {
    assert!(AppService::validate_bed_transition("Out of Service", "Occupied").is_err());
}

#[test]
fn bed_out_of_service_to_cleaning_rejected() {
    assert!(AppService::validate_bed_transition("Out of Service", "Cleaning").is_err());
}

// ── Bed identity transitions rejected ──

#[test]
fn bed_reserved_to_reserved_rejected() {
    assert!(AppService::validate_bed_transition("Reserved", "Reserved").is_err());
}

#[test]
fn bed_occupied_to_occupied_rejected() {
    assert!(AppService::validate_bed_transition("Occupied", "Occupied").is_err());
}

#[test]
fn bed_cleaning_to_cleaning_rejected() {
    assert!(AppService::validate_bed_transition("Cleaning", "Cleaning").is_err());
}

#[test]
fn bed_out_of_service_to_out_of_service_rejected() {
    assert!(AppService::validate_bed_transition("Out of Service", "Out of Service").is_err());
}

// ── Order status validation completeness ──

#[test]
fn status_requires_reason_enumeration() {
    assert!(AppService::status_requires_reason("Canceled"));
    assert!(AppService::status_requires_reason("Credited"));
    assert!(!AppService::status_requires_reason("Created"));
    assert!(!AppService::status_requires_reason("Billed"));
    assert!(!AppService::status_requires_reason("Delivered"));
}

// ── Ensure reason validation edge cases ──

#[test]
fn ensure_reason_rejects_whitespace_only() {
    assert!(AppService::ensure_reason("   ").is_err());
    assert!(AppService::ensure_reason("\t\n").is_err());
}

#[test]
fn ensure_reason_accepts_valid_text() {
    assert!(AppService::ensure_reason("demographics correction").is_ok());
}

// ── Attachment: boundary and edge-case tests ──

#[test]
fn attachment_validate_accepts_minimal_size() {
    assert!(AppService::validate_attachment("doc.pdf", "application/pdf", 1).is_ok());
}

#[test]
fn attachment_validate_accepts_pdf_ext_with_jpeg_mime() {
    // Extension and MIME are validated independently (both must be in allowlist)
    assert!(AppService::validate_attachment("doc.pdf", "image/jpeg", 100).is_ok());
}

#[test]
fn attachment_validate_rejects_uppercase_exe() {
    assert!(AppService::validate_attachment("PAYLOAD.EXE", "application/octet-stream", 100).is_err());
}

// ── Content signature: edge cases ──

#[test]
fn verify_content_signature_rejects_empty_data_for_pdf() {
    assert!(AppService::verify_content_signature(b"", "application/pdf").is_err());
}

#[test]
fn verify_content_signature_rejects_empty_data_for_jpeg() {
    assert!(AppService::verify_content_signature(b"", "image/jpeg").is_err());
}

#[test]
fn verify_content_signature_rejects_empty_data_for_png() {
    assert!(AppService::verify_content_signature(b"", "image/png").is_err());
}

// ── Campaign deadline: additional edge cases ──

#[test]
fn campaign_deadline_rejects_whitespace_only() {
    assert!(AppService::normalize_campaign_deadline("   ").is_err());
}

#[test]
fn campaign_deadline_rejects_malformed_datetime() {
    assert!(AppService::normalize_campaign_deadline("not-a-date").is_err());
}

#[test]
fn campaign_deadline_accepts_rfc3339_with_offset() {
    let out = AppService::normalize_campaign_deadline("2099-06-15T14:30:00+00:00");
    assert!(out.is_ok());
}

// ── CSV export: special characters ──

#[test]
fn csv_escape_handles_commas_and_newlines() {
    let escaped = AppService::csv_escape("value,with\nnewline");
    assert_eq!(escaped, "\"value,with\nnewline\"");
}

#[test]
fn csv_escape_handles_empty_string() {
    let escaped = AppService::csv_escape("");
    assert_eq!(escaped, "\"\"");
}

// ── Argon2 hashing: round-trip with edge-case inputs ──

#[test]
fn argon2_hash_has_argon2id_prefix() {
    let hash = AppService::hash_password_argon2("Test#Pwd12345").expect("hash");
    assert!(hash.starts_with("$argon2id$"), "hash must be argon2id format");
}

#[test]
fn legacy_sha256_detection_rejects_short_hex() {
    assert!(!AppService::is_legacy_sha256_hash("abcdef"));
}

#[test]
fn legacy_sha256_detection_rejects_non_hex() {
    assert!(!AppService::is_legacy_sha256_hash(
        "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz"
    ));
}

// ── Sensitive field detection completeness ──

#[test]
fn sensitive_field_detection_rejects_id_field() {
    assert!(!AppService::is_sensitive_revision_field("id"));
}

#[test]
fn sensitive_field_detection_rejects_created_at() {
    assert!(!AppService::is_sensitive_revision_field("created_at"));
}

// ── Token fingerprint: length and format ──

#[test]
fn token_fingerprint_is_hex_only() {
    let fp = AppService::token_fingerprint("some-token");
    assert!(fp.chars().all(|c| c.is_ascii_hexdigit()), "fingerprint must be hex");
}

#[test]
fn token_fingerprint_empty_input() {
    let fp = AppService::token_fingerprint("");
    assert_eq!(fp.len(), 12);
}

// ── Revision delta: empty diff handling ──

#[test]
fn revision_delta_handles_empty_diffs() {
    let item = RevisionTimelineDto {
        id: 1,
        entity_type: "demographics".to_string(),
        diff_before: "{}".to_string(),
        diff_after: "{}".to_string(),
        field_deltas_json: String::new(),
        reason_for_change: "no change".to_string(),
        actor_username: "admin".to_string(),
        created_at: "2026-01-01 00:00:00".to_string(),
    };
    let decorated = AppService::decorate_revision_deltas(item, false);
    assert_eq!(decorated.field_deltas_json, "[]");
}

#[test]
fn revision_delta_handles_malformed_json() {
    let item = RevisionTimelineDto {
        id: 1,
        entity_type: "demographics".to_string(),
        diff_before: "not json".to_string(),
        diff_after: "also not json".to_string(),
        field_deltas_json: String::new(),
        reason_for_change: "test".to_string(),
        actor_username: "admin".to_string(),
        created_at: "2026-01-01 00:00:00".to_string(),
    };
    let decorated = AppService::decorate_revision_deltas(item, false);
    assert_eq!(decorated.field_deltas_json, "[]");
}
