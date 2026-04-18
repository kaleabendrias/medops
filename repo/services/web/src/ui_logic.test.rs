use crate::ui_logic::{
    can_submit_upload, parse_revision_deltas, queue_dropped_attachment, upload_state_after_attempt,
    validate_attachment, QueuedAttachment, UploadState,
};

#[test]
fn validate_jpeg_extensions_both_spellings() {
    assert!(validate_attachment("photo.jpg", "image/jpeg", 512).is_ok());
    assert!(validate_attachment("photo.jpeg", "image/jpeg", 512).is_ok());
}

#[test]
fn validate_rejects_pdf_extension_with_jpeg_mime() {
    assert!(validate_attachment("doc.pdf", "image/jpeg", 512).is_err());
}

#[test]
fn queue_rejected_attachment_is_not_added() {
    let mut q: Vec<QueuedAttachment> = Vec::new();
    let result =
        queue_dropped_attachment(&mut q, "virus.exe", "application/octet-stream", vec![0u8; 100]);
    assert!(result.is_err());
    assert!(q.is_empty());
}

#[test]
fn upload_state_success_from_uploading() {
    assert_eq!(upload_state_after_attempt(UploadState::Uploading, true), UploadState::Success);
}

#[test]
fn upload_state_success_from_idle() {
    assert_eq!(upload_state_after_attempt(UploadState::Idle, true), UploadState::Success);
}

#[test]
fn upload_state_failed_only_when_previously_uploading() {
    assert_eq!(
        upload_state_after_attempt(UploadState::Success, false),
        UploadState::Success
    );
    assert_eq!(
        upload_state_after_attempt(UploadState::Uploading, false),
        UploadState::Failed
    );
}

#[test]
fn cannot_submit_upload_without_queued_files() {
    assert!(!can_submit_upload(Some(99), false, 0));
}

#[test]
fn revision_deltas_sensitive_flag_preserved() {
    let json =
        r#"[{"field":"allergies","before":"none","after":"peanuts","sensitive":true}]"#;
    let parsed = parse_revision_deltas(json);
    assert_eq!(parsed.len(), 1);
    assert!(parsed[0].sensitive);
    assert_eq!(parsed[0].field, "allergies");
}

#[test]
fn validate_at_exactly_25mb_limit_succeeds() {
    let exactly = 25 * 1024 * 1024;
    assert!(validate_attachment("scan.pdf", "application/pdf", exactly).is_ok());
}

#[test]
fn validate_one_byte_over_25mb_fails() {
    let over = 25 * 1024 * 1024 + 1;
    assert!(validate_attachment("scan.pdf", "application/pdf", over).is_err());
}

#[test]
fn cannot_submit_without_patient_id() {
    assert!(!can_submit_upload(None, false, 3));
}

#[test]
fn cannot_submit_while_uploading_even_with_files() {
    assert!(!can_submit_upload(Some(1), true, 5));
}

#[test]
fn revision_deltas_with_multiple_sensitive_and_non_sensitive_fields() {
    let json = r#"[
        {"field":"first_name","before":"Alice","after":"Bob","sensitive":false},
        {"field":"allergies","before":"none","after":"peanuts","sensitive":true}
    ]"#;
    let parsed = parse_revision_deltas(json);
    assert_eq!(parsed.len(), 2);
    assert!(!parsed[0].sensitive);
    assert!(parsed[1].sensitive);
}

#[test]
fn validate_case_insensitive_extension_matching() {
    assert!(validate_attachment("SCAN.PDF", "application/pdf", 100).is_ok());
    assert!(validate_attachment("PHOTO.JPG", "image/jpeg", 100).is_ok());
    assert!(validate_attachment("IMAGE.PNG", "image/png", 100).is_ok());
}
