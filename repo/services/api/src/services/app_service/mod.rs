use std::sync::Arc;

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::Engine;
use chrono::{DateTime, NaiveDateTime, Utc};
use contracts::{
    AttachmentMetadataDto, AuditLogDto, AuthLoginRequest, AuthLoginResponse, BedDto,
    BedEventDto, BedTransitionRequest, CampaignCreateRequest, CampaignDto, ClinicalEditRequest,
    DiningMenuDto, DiningMenuRequest, DishCategoryDto, DishCreateRequest, DishDto,
    DishOptionRequest, DishStatusRequest, DishWindowRequest, ExperimentAssignRequest,
    ExperimentBacktrackRequest, ExperimentCreateRequest, ExperimentVariantRequest,
    FunnelMetricsDto, GovernanceDeleteRequest, GovernanceRecordDto, GovernanceRecordRequest,
    HospitalDto, IngestionTaskCreateRequest, IngestionTaskDto, IngestionTaskRollbackRequest,
    IngestionTaskRunDto, IngestionTaskUpdateRequest, IngestionTaskVersionDto, MenuEntitlementDto,
    OrderCreateRequest, OrderDto, OrderNoteDto, OrderNoteRequest, OrderStatusRequest,
    PatientCreateRequest, PatientExportDto,
    PatientProfileDto, PatientSearchResultDto, PatientUpdateRequest, RankingRuleDto,
    RankingRuleRequest, RecommendationKpiDto, RecommendationDto, RetentionMetricsDto,
    RetentionPolicyDto, RevisionTimelineDto, RoleDto, TelemetryEventRequest,
    TicketSplitDto, TicketSplitRequest, UserSummaryDto, VisitNoteRequest,
};
use rand::RngCore;
use regex::Regex;
use sha2::Digest;

use crate::contracts::{ApiError, AuthUser};
use crate::config::{AuthPolicyConfig, RetentionConfig};
use crate::repositories::app_repository::{AppRepository, BedTransitionDbRequest, OrderRecord};

mod auth;
mod clinical;
mod dining;
mod governance;

#[derive(Clone)]
pub struct AppService {
    pub(crate) repo: Arc<dyn AppRepository>,
    pub(crate) password_min_length: u32,
    pub(crate) lockout_minutes: u32,
    pub(crate) session_inactivity_minutes: u32,
    pub(crate) clinical_years_min: u32,
}

impl AppService {
    pub fn new(repo: Arc<dyn AppRepository>, auth_policy: &AuthPolicyConfig, retention: &RetentionConfig) -> Self {
        Self {
            repo,
            password_min_length: auth_policy.password_min_length,
            lockout_minutes: auth_policy.lockout_minutes,
            session_inactivity_minutes: auth_policy.session_inactivity_minutes,
            clinical_years_min: retention.clinical_years_min,
        }
    }

    fn validate_password_complexity(&self, password: &str) -> Result<(), ApiError> {
        Self::validate_password_complexity_with_min(password, self.password_min_length)
    }

    pub(crate) fn validate_password_complexity_with_min(password: &str, min_len: u32) -> Result<(), ApiError> {
        if password.len() < min_len as usize {
            return Err(ApiError::bad_request(&format!(
                "Password must be at least {} characters with upper, lower, number, and symbol",
                min_len
            )));
        }
        let has_upper = password.chars().any(|c| c.is_ascii_uppercase());
        let has_lower = password.chars().any(|c| c.is_ascii_lowercase());
        let has_digit = password.chars().any(|c| c.is_ascii_digit());
        let has_symbol = password.chars().any(|c| !c.is_ascii_alphanumeric());
        if !(has_upper && has_lower && has_digit && has_symbol) {
            return Err(ApiError::bad_request(
                "Password must include upper, lower, number, and symbol",
            ));
        }
        Ok(())
    }

    pub(crate) fn hash_password_argon2(password: &str) -> Result<String, ApiError> {
        let params = Params::new(19_456, 2, 1, Some(32)).map_err(|_| ApiError::Internal)?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let salt = SaltString::generate(&mut OsRng);
        argon2
            .hash_password(password.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(|_| ApiError::Internal)
    }

    pub(crate) fn is_legacy_sha256_hash(stored_hash: &str) -> bool {
        let Ok(re) = Regex::new(r"^[a-f0-9]{64}$") else {
            return false;
        };
        re.is_match(stored_hash)
    }

    pub(crate) fn verify_password(password: &str, stored_hash: &str) -> Result<(bool, bool), ApiError> {
        if Self::is_legacy_sha256_hash(stored_hash) {
            let legacy = hex::encode(sha2::Sha256::digest(password.as_bytes()));
            return Ok((legacy == stored_hash, legacy == stored_hash));
        }

        let parsed = PasswordHash::new(stored_hash).map_err(|_| ApiError::Unauthorized)?;
        let params = Params::new(19_456, 2, 1, Some(32)).map_err(|_| ApiError::Internal)?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        Ok((argon2.verify_password(password.as_bytes(), &parsed).is_ok(), false))
    }

    pub(crate) fn generate_session_token() -> String {
        let mut bytes = [0u8; 48];
        rand::thread_rng().fill_bytes(&mut bytes);
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    }

    pub(crate) fn ensure_reason(reason: &str) -> Result<(), ApiError> {
        if reason.trim().is_empty() {
            return Err(ApiError::bad_request("Reason for change is required"));
        }
        Ok(())
    }

    pub(crate) fn validate_attachment(filename: &str, mime_type: &str, size_bytes: i64) -> Result<(), ApiError> {
        let lower = filename.to_ascii_lowercase();
        let ext_ok = lower.ends_with(".pdf") || lower.ends_with(".jpg") || lower.ends_with(".jpeg") || lower.ends_with(".png");
        let mime_ok = matches!(mime_type, "application/pdf" | "image/jpeg" | "image/png");
        if !ext_ok || !mime_ok {
            return Err(ApiError::bad_request("Only PDF, JPG, and PNG files are allowed"));
        }
        if size_bytes > 25 * 1024 * 1024 {
            return Err(ApiError::PayloadTooLarge);
        }
        Ok(())
    }

    /// Verify that the first bytes of the uploaded file match the expected
    /// magic-byte signature for the declared MIME type.  This prevents
    /// extension/MIME spoofing where the actual file content is a different type.
    pub(crate) fn verify_content_signature(data: &[u8], mime_type: &str) -> Result<(), ApiError> {
        let ok = match mime_type {
            "application/pdf" => data.starts_with(b"%PDF"),
            "image/jpeg" => data.starts_with(&[0xFF, 0xD8, 0xFF]),
            "image/png" => data.starts_with(&[0x89, 0x50, 0x4E, 0x47]),
            _ => false,
        };
        if !ok {
            return Err(ApiError::bad_request(
                "File content does not match declared MIME type (magic-byte mismatch)",
            ));
        }
        Ok(())
    }

    pub(crate) fn validate_bed_transition(current: &str, target: &str) -> Result<(), ApiError> {
        let valid = match current {
            "Available" => ["Reserved", "Occupied", "Out of Service"].as_slice(),
            "Reserved" => ["Occupied", "Available", "Out of Service"].as_slice(),
            "Occupied" => ["Cleaning", "Reserved"].as_slice(),
            "Cleaning" => ["Available", "Out of Service"].as_slice(),
            "Out of Service" => ["Available"].as_slice(),
            _ => return Err(ApiError::bad_request("Unknown current bed state")),
        };
        if !valid.contains(&target) {
            return Err(ApiError::bad_request("Invalid bed state transition"));
        }
        Ok(())
    }

    pub(crate) fn status_requires_reason(status: &str) -> bool {
        matches!(status, "Canceled" | "Credited")
    }

    pub(crate) fn token_fingerprint(token: &str) -> String {
        let digest = sha2::Sha256::digest(token.as_bytes());
        let hex = hex::encode(digest);
        hex[..12].to_string()
    }

    pub(crate) fn error_code(err: &ApiError) -> &'static str {
        match err {
            ApiError::Unauthorized => "unauthorized",
            ApiError::Forbidden => "forbidden",
            ApiError::NotFound => "not_found",
            ApiError::Conflict => "conflict",
            ApiError::BadRequest(_) => "bad_request",
            ApiError::PayloadTooLarge => "payload_too_large",
            ApiError::Database(_) => "database",
            ApiError::Migrate(_) => "migrate",
            ApiError::Internal => "internal",
        }
    }

    pub(crate) fn is_sensitive_revision_field(field: &str) -> bool {
        matches!(field, "mrn" | "allergies" | "contraindications" | "history")
    }

    pub(crate) fn parse_object(json_blob: &str) -> serde_json::Map<String, serde_json::Value> {
        serde_json::from_str::<serde_json::Value>(json_blob)
            .ok()
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default()
    }

    pub(crate) fn stringify_delta_value(value: &serde_json::Value) -> String {
        value
            .as_str()
            .map(ToString::to_string)
            .unwrap_or_else(|| value.to_string())
    }

    pub(crate) fn decorate_revision_deltas(mut item: RevisionTimelineDto, reveal_sensitive: bool) -> RevisionTimelineDto {
        let before_map = Self::parse_object(&item.diff_before);
        let after_map = Self::parse_object(&item.diff_after);
        let mut keys = before_map.keys().cloned().collect::<Vec<_>>();
        keys.extend(after_map.keys().cloned());
        keys.sort();
        keys.dedup();

        let mut deltas = Vec::new();
        for key in keys {
            let before = before_map
                .get(&key)
                .map(Self::stringify_delta_value)
                .unwrap_or_default();
            let after = after_map
                .get(&key)
                .map(Self::stringify_delta_value)
                .unwrap_or_default();
            if before == after {
                continue;
            }

            let sensitive = Self::is_sensitive_revision_field(&key);
            let before_out = if sensitive && !reveal_sensitive {
                "[REDACTED - privileged reveal required]".to_string()
            } else {
                before
            };
            let after_out = if sensitive && !reveal_sensitive {
                "[REDACTED - privileged reveal required]".to_string()
            } else {
                after
            };
            deltas.push(serde_json::json!({
                "field": key,
                "before": before_out,
                "after": after_out,
                "sensitive": sensitive
            }));
        }

        item.field_deltas_json = serde_json::to_string(&deltas).unwrap_or_else(|_| "[]".to_string());

        // Redact raw diff payloads for non-privileged clients to prevent
        // sensitive data leakage through the revision API.
        if !reveal_sensitive {
            item.diff_before = "{}".to_string();
            item.diff_after = "{}".to_string();
        }

        item
    }

    pub(crate) fn security_log(event: &str, outcome: &str, details: serde_json::Value) {
        let sanitized = crate::infrastructure::logging::sanitize_details(&details);
        tracing::info!(
            category = "security",
            event = event,
            outcome = outcome,
            details = %sanitized,
            "security_event"
        );
    }

    pub(crate) fn normalize_campaign_deadline(input: &str) -> Result<String, ApiError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(ApiError::bad_request("success_deadline_at is required"));
        }

        let parsed = DateTime::parse_from_rfc3339(trimmed)
            .map(|dt| dt.with_timezone(&Utc).naive_utc())
            .or_else(|_| NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S"))
            .map_err(|_| {
                ApiError::bad_request(
                    "success_deadline_at must be RFC3339 or 'YYYY-MM-DD HH:MM:SS' in UTC",
                )
            })?;

        if parsed <= Utc::now().naive_utc() {
            return Err(ApiError::bad_request("success_deadline_at must be in the future"));
        }
        Ok(parsed.format("%Y-%m-%d %H:%M:%S").to_string())
    }

    pub(crate) fn csv_escape(value: &str) -> String {
        let escaped = value.replace('"', "\"\"");
        format!("\"{}\"", escaped)
    }

    pub(crate) fn patient_as_csv(patient: &PatientProfileDto) -> String {
        let header = "id,mrn,first_name,last_name,birth_date,gender,phone,email,allergies,contraindications,history";
        let row = [
            patient.id.to_string(),
            Self::csv_escape(&patient.mrn),
            Self::csv_escape(&patient.first_name),
            Self::csv_escape(&patient.last_name),
            Self::csv_escape(&patient.birth_date),
            Self::csv_escape(&patient.gender),
            Self::csv_escape(&patient.phone),
            Self::csv_escape(&patient.email),
            Self::csv_escape(&patient.allergies),
            Self::csv_escape(&patient.contraindications),
            Self::csv_escape(&patient.history),
        ]
        .join(",");
        format!("{header}\n{row}\n")
    }
}

#[cfg(test)]
mod tests;
