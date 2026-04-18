use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueuedAttachment {
    pub file_name: String,
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UploadState {
    Idle,
    Uploading,
    Success,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct RevisionDelta {
    pub field: String,
    pub before: String,
    pub after: String,
    pub sensitive: bool,
}

pub fn parse_revision_deltas(field_deltas_json: &str) -> Vec<RevisionDelta> {
    serde_json::from_str(field_deltas_json).unwrap_or_default()
}

pub fn validate_attachment(file_name: &str, mime_type: &str, size_bytes: usize) -> Result<(), String> {
    let lower = file_name.to_ascii_lowercase();
    let exact_match = (lower.ends_with(".pdf") && mime_type == "application/pdf")
        || ((lower.ends_with(".jpg") || lower.ends_with(".jpeg"))
            && mime_type == "image/jpeg")
        || (lower.ends_with(".png") && mime_type == "image/png");
    if !exact_match {
        return Err("Attachment must be PDF/JPG/PNG with matching MIME type".to_string());
    }
    if size_bytes > 25 * 1024 * 1024 {
        return Err("Attachment exceeds 25 MB limit".to_string());
    }
    Ok(())
}

pub fn queue_dropped_attachment(
    queue: &mut Vec<QueuedAttachment>,
    file_name: &str,
    mime_type: &str,
    bytes: Vec<u8>,
) -> Result<(), String> {
    validate_attachment(file_name, mime_type, bytes.len())?;
    if !queue
        .iter()
        .any(|item| item.file_name == file_name && item.mime_type == mime_type && item.bytes.len() == bytes.len())
    {
        queue.push(QueuedAttachment {
            file_name: file_name.to_string(),
            mime_type: mime_type.to_string(),
            bytes,
        });
    }
    Ok(())
}

pub fn can_submit_upload(
    selected_patient_id: Option<i64>,
    uploading: bool,
    queued_count: usize,
) -> bool {
    selected_patient_id.is_some() && !uploading && queued_count > 0
}

pub fn upload_state_after_attempt(previous: UploadState, success: bool) -> UploadState {
    if success {
        UploadState::Success
    } else if matches!(previous, UploadState::Uploading) {
        UploadState::Failed
    } else {
        previous
    }
}

#[cfg(test)]
mod tests {
    use super::{
        can_submit_upload, parse_revision_deltas, queue_dropped_attachment,
        upload_state_after_attempt, validate_attachment, QueuedAttachment, UploadState,
    };

    #[test]
    fn validates_allowed_attachment() {
        assert!(validate_attachment("scan.pdf", "application/pdf", 1024).is_ok());
    }

    #[test]
    fn rejects_bad_mime_for_extension() {
        assert!(validate_attachment("scan.pdf", "image/png", 1024).is_err());
    }

    #[test]
    fn rejects_oversized_attachment() {
        assert!(validate_attachment("scan.png", "image/png", 30 * 1024 * 1024).is_err());
    }

    #[test]
    fn accepts_attachment_exactly_at_limit() {
        assert!(validate_attachment("scan.png", "image/png", 25 * 1024 * 1024).is_ok());
    }

    #[test]
    fn queues_unique_attachment_once() {
        let mut queue: Vec<QueuedAttachment> = Vec::new();
        let first = queue_dropped_attachment(
            &mut queue,
            "lab.pdf",
            "application/pdf",
            b"payload".to_vec(),
        );
        let second = queue_dropped_attachment(
            &mut queue,
            "lab.pdf",
            "application/pdf",
            b"payload".to_vec(),
        );
        assert!(first.is_ok());
        assert!(second.is_ok());
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn prevents_duplicate_click_while_uploading() {
        assert!(!can_submit_upload(Some(1), true, 1));
    }

    #[test]
    fn allows_submit_when_ready() {
        assert!(can_submit_upload(Some(1), false, 1));
    }

    #[test]
    fn async_failure_recovery_marks_failed_state() {
        assert_eq!(
            upload_state_after_attempt(UploadState::Uploading, false),
            UploadState::Failed
        );
    }

    #[test]
    fn async_success_marks_success_state() {
        assert_eq!(
            upload_state_after_attempt(UploadState::Uploading, true),
            UploadState::Success
        );
    }

    #[test]
    fn parses_revision_deltas_payload() {
        let parsed = parse_revision_deltas(
            "[{\"field\":\"first_name\",\"before\":\"Old\",\"after\":\"New\",\"sensitive\":false}]",
        );
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].field, "first_name");
        assert_eq!(parsed[0].after, "New");
    }

    #[test]
    fn malformed_revision_deltas_fall_back_to_empty() {
        let parsed = parse_revision_deltas("not-json");
        assert!(parsed.is_empty());
    }

    #[test]
    fn rejects_unsupported_extension() {
        assert!(validate_attachment("file.exe", "application/octet-stream", 1024).is_err());
        assert!(validate_attachment("file.docx", "application/vnd.openxmlformats", 1024).is_err());
        assert!(validate_attachment("file.txt", "text/plain", 1024).is_err());
    }

    #[test]
    fn rejects_attachment_one_byte_over_limit() {
        let over_limit = 25 * 1024 * 1024 + 1;
        assert!(validate_attachment("scan.png", "image/png", over_limit).is_err());
    }

    #[test]
    fn accepts_attachment_one_byte_under_limit() {
        let under_limit = 25 * 1024 * 1024 - 1;
        assert!(validate_attachment("scan.png", "image/png", under_limit).is_ok());
    }

    #[test]
    fn accepts_zero_byte_attachment() {
        assert!(validate_attachment("scan.png", "image/png", 0).is_ok());
    }

    #[test]
    fn queues_different_files_separately() {
        let mut queue: Vec<QueuedAttachment> = Vec::new();
        queue_dropped_attachment(&mut queue, "a.pdf", "application/pdf", b"aaa".to_vec()).unwrap();
        queue_dropped_attachment(&mut queue, "b.pdf", "application/pdf", b"bbb".to_vec()).unwrap();
        assert_eq!(queue.len(), 2);
    }

    #[test]
    fn cannot_submit_without_patient_selected() {
        assert!(!can_submit_upload(None, false, 1));
    }

    #[test]
    fn cannot_submit_with_empty_queue() {
        assert!(!can_submit_upload(Some(1), false, 0));
    }

    #[test]
    fn idle_state_after_failed_non_uploading_attempt_stays_same() {
        assert_eq!(
            upload_state_after_attempt(UploadState::Idle, false),
            UploadState::Idle
        );
    }

    #[test]
    fn revision_deltas_empty_json_array_parses_to_empty() {
        let parsed = parse_revision_deltas("[]");
        assert!(parsed.is_empty());
    }

    #[test]
    fn revision_deltas_multiple_entries() {
        let json = r#"[
            {"field":"first_name","before":"A","after":"B","sensitive":false},
            {"field":"allergies","before":"none","after":"shellfish","sensitive":true}
        ]"#;
        let parsed = parse_revision_deltas(json);
        assert_eq!(parsed.len(), 2);
        assert!(!parsed[0].sensitive);
        assert!(parsed[1].sensitive);
    }

    #[test]
    fn validate_attachment_case_insensitive_extension() {
        assert!(validate_attachment("scan.PDF", "application/pdf", 1024).is_ok());
        assert!(validate_attachment("photo.JPG", "image/jpeg", 1024).is_ok());
        assert!(validate_attachment("image.PNG", "image/png", 1024).is_ok());
    }
}

#[cfg(test)]
#[path = "ui_logic.test.rs"]
mod ui_logic_extended_tests;
