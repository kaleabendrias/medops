use contracts::{PatientUpdateRequest, RevisionTimelineDto};

use crate::features::revisions::deltas_for;

pub struct DemographicsForm<'a> {
    pub first_name: &'a str,
    pub last_name: &'a str,
    pub birth_date: &'a str,
    pub gender: &'a str,
    pub phone: &'a str,
    pub email: &'a str,
    pub reason_for_change: &'a str,
}

pub fn build_demographics_request(form: DemographicsForm<'_>) -> Result<PatientUpdateRequest, String> {
    if form.reason_for_change.trim().is_empty() {
        return Err("Reason for change is required".to_string());
    }

    Ok(PatientUpdateRequest {
        first_name: form.first_name.trim().to_string(),
        last_name: form.last_name.trim().to_string(),
        birth_date: form.birth_date.trim().to_string(),
        gender: form.gender.trim().to_string(),
        phone: form.phone.trim().to_string(),
        email: form.email.trim().to_string(),
        reason_for_change: form.reason_for_change.trim().to_string(),
    })
}

pub fn revisions_include_sensitive_values(
    revisions: &[RevisionTimelineDto],
    sensitive_field: &str,
) -> bool {
    revisions.iter().any(|rev| {
        deltas_for(rev).into_iter().any(|delta| {
            delta.field == sensitive_field
                && delta.before != "[REDACTED - privileged reveal required]"
                && delta.after != "[REDACTED - privileged reveal required]"
        })
    })
}

#[cfg(test)]
mod tests {
    use contracts::RevisionTimelineDto;

    use super::{build_demographics_request, revisions_include_sensitive_values, DemographicsForm};

    #[test]
    fn clinical_edit_requires_reason_for_change() {
        let req = build_demographics_request(DemographicsForm {
            first_name: "Ana",
            last_name: "Doe",
            birth_date: "1990-01-01",
            gender: "F",
            phone: "555",
            email: "ana@example.local",
            reason_for_change: " ",
        });
        assert!(req.is_err());
    }

    #[test]
    fn revision_visibility_detects_masked_sensitive_deltas() {
        let masked = RevisionTimelineDto {
            id: 1,
            entity_type: "clinical".to_string(),
            diff_before: "{}".to_string(),
            diff_after: "{}".to_string(),
            field_deltas_json: "[{\"field\":\"allergies\",\"before\":\"[REDACTED - privileged reveal required]\",\"after\":\"[REDACTED - privileged reveal required]\",\"sensitive\":true}]".to_string(),
            reason_for_change: "update".to_string(),
            actor_username: "doctor".to_string(),
            created_at: "2026-01-01 00:00:00".to_string(),
        };
        assert!(!revisions_include_sensitive_values(&[masked], "allergies"));
    }

    #[test]
    fn revision_visibility_detects_revealed_sensitive_deltas() {
        let revealed = RevisionTimelineDto {
            id: 1,
            entity_type: "clinical".to_string(),
            diff_before: "{}".to_string(),
            diff_after: "{}".to_string(),
            field_deltas_json:
                "[{\"field\":\"allergies\",\"before\":\"none\",\"after\":\"peanut\",\"sensitive\":true}]"
                    .to_string(),
            reason_for_change: "update".to_string(),
            actor_username: "doctor".to_string(),
            created_at: "2026-01-01 00:00:00".to_string(),
        };
        assert!(revisions_include_sensitive_values(&[revealed], "allergies"));
    }
}
