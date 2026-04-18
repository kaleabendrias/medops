use sha2::{Digest, Sha256};

use crate::contracts::ApiError;
use crate::infrastructure::security::field_crypto::FieldCrypto;
use crate::repositories::app_repository::{AppRepository, PatientSensitiveRecord};
use contracts::PatientProfileDto;

mod clinical;
mod dining;
mod dispatch;
mod governance;
mod ingestion;

pub struct MySqlAppRepository {
    pool: sqlx::MySqlPool,
    lockout_failed_attempts: i32,
    lockout_minutes: i32,
    session_inactivity_minutes: i32,
    field_crypto: FieldCrypto,
}

impl MySqlAppRepository {
    pub fn new(
        pool: sqlx::MySqlPool,
        lockout_failed_attempts: i32,
        lockout_minutes: i32,
        session_inactivity_minutes: i32,
        field_crypto: FieldCrypto,
    ) -> Self {
        Self {
            pool,
            lockout_failed_attempts,
            lockout_minutes,
            session_inactivity_minutes,
            field_crypto,
        }
    }

    /// Hash a session bearer token using SHA-256 and return the hex digest.
    /// The plaintext token is generated client-side and never persisted; only
    /// this digest goes to the database, so a read of the `sessions` table
    /// cannot be replayed against the API to hijack a session.
    fn hash_session_token(token: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Capability check for "this role can see/operate on patients owned by
    /// any user, not just the ones it has been individually assigned to".
    pub(crate) async fn has_global_patient_access(&self, role_name: &str) -> Result<bool, ApiError> {
        self.user_has_permission(role_name, "patient.global_access").await
    }

    /// Capability check for "this role can see/operate on dining orders
    /// created by any user".
    pub(crate) async fn has_global_order_access(&self, role_name: &str) -> Result<bool, ApiError> {
        self.user_has_permission(role_name, "order.global_access").await
    }

    /// Capability check for "this role can see/operate on ingestion tasks
    /// created by any user".
    pub(crate) async fn has_global_ingestion_access(&self, role_name: &str) -> Result<bool, ApiError> {
        self.user_has_permission(role_name, "ingestion.global_access").await
    }

    /// Stateless bed state-machine validation.
    pub(crate) fn validate_bed_state_transition(current: &str, target: &str) -> Result<(), ApiError> {
        let valid: &[&str] = match current {
            "Available" => &["Reserved", "Occupied", "Out of Service"],
            "Reserved" => &["Occupied", "Available", "Out of Service"],
            "Occupied" => &["Cleaning", "Reserved"],
            "Cleaning" => &["Available", "Out of Service"],
            "Out of Service" => &["Available"],
            _ => return Err(ApiError::bad_request("Unknown current bed state")),
        };
        if !valid.contains(&target) {
            return Err(ApiError::bad_request("Invalid bed state transition"));
        }
        Ok(())
    }

    fn mask_mrn(value: &str) -> String {
        let chars: Vec<char> = value.chars().collect();
        if chars.len() <= 4 {
            return "****".to_string();
        }
        let last4: String = chars[chars.len() - 4..].iter().collect();
        format!("***{}", last4)
    }

    fn mask_long(value: &str) -> String {
        if value.trim().is_empty() {
            return String::new();
        }
        "[REDACTED - privileged reveal required]".to_string()
    }

    pub(crate) fn to_masked_patient(row: PatientSensitiveRecord) -> PatientProfileDto {
        PatientProfileDto {
            id: row.id,
            mrn: Self::mask_mrn(&row.mrn),
            first_name: row.first_name,
            last_name: row.last_name,
            birth_date: row.birth_date,
            gender: row.gender,
            phone: row.phone,
            email: row.email,
            allergies: Self::mask_long(&row.allergies),
            contraindications: Self::mask_long(&row.contraindications),
            history: Self::mask_long(&row.history),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MySqlAppRepository;

    #[test]
    fn hash_session_token_is_deterministic() {
        let h1 = MySqlAppRepository::hash_session_token("some-bearer-token");
        let h2 = MySqlAppRepository::hash_session_token("some-bearer-token");
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_session_token_output_is_64_hex_chars() {
        let h = MySqlAppRepository::hash_session_token("test-token-value");
        assert_eq!(h.len(), 64, "SHA-256 hex digest must be 64 characters");
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hash_session_token_different_inputs_produce_different_digests() {
        let h1 = MySqlAppRepository::hash_session_token("token-a");
        let h2 = MySqlAppRepository::hash_session_token("token-b");
        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_session_token_empty_string_is_64_hex_chars() {
        let h = MySqlAppRepository::hash_session_token("");
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hash_session_token_is_consistent_with_sha2_crate() {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(b"abc");
        let expected = hex::encode(hasher.finalize());
        let actual = MySqlAppRepository::hash_session_token("abc");
        assert_eq!(actual, expected, "hash_session_token must match direct sha2::Sha256 computation");
    }

    #[test]
    fn bed_state_available_allows_reserved_occupied_and_out_of_service() {
        assert!(MySqlAppRepository::validate_bed_state_transition("Available", "Reserved").is_ok());
        assert!(MySqlAppRepository::validate_bed_state_transition("Available", "Occupied").is_ok());
        assert!(MySqlAppRepository::validate_bed_state_transition("Available", "Out of Service").is_ok());
    }

    #[test]
    fn bed_state_available_rejects_cleaning_and_self() {
        assert!(MySqlAppRepository::validate_bed_state_transition("Available", "Cleaning").is_err());
        assert!(MySqlAppRepository::validate_bed_state_transition("Available", "Available").is_err());
    }

    #[test]
    fn bed_state_occupied_allows_cleaning_and_reserved() {
        assert!(MySqlAppRepository::validate_bed_state_transition("Occupied", "Cleaning").is_ok());
        assert!(MySqlAppRepository::validate_bed_state_transition("Occupied", "Reserved").is_ok());
    }

    #[test]
    fn bed_state_occupied_rejects_available_and_out_of_service() {
        assert!(MySqlAppRepository::validate_bed_state_transition("Occupied", "Available").is_err());
        assert!(MySqlAppRepository::validate_bed_state_transition("Occupied", "Out of Service").is_err());
    }

    #[test]
    fn bed_state_cleaning_allows_available_and_out_of_service() {
        assert!(MySqlAppRepository::validate_bed_state_transition("Cleaning", "Available").is_ok());
        assert!(MySqlAppRepository::validate_bed_state_transition("Cleaning", "Out of Service").is_ok());
    }

    #[test]
    fn bed_state_cleaning_rejects_occupied_and_reserved() {
        assert!(MySqlAppRepository::validate_bed_state_transition("Cleaning", "Occupied").is_err());
        assert!(MySqlAppRepository::validate_bed_state_transition("Cleaning", "Reserved").is_err());
    }

    #[test]
    fn bed_state_out_of_service_allows_only_available() {
        assert!(MySqlAppRepository::validate_bed_state_transition("Out of Service", "Available").is_ok());
        assert!(MySqlAppRepository::validate_bed_state_transition("Out of Service", "Reserved").is_err());
        assert!(MySqlAppRepository::validate_bed_state_transition("Out of Service", "Occupied").is_err());
    }

    #[test]
    fn bed_state_unknown_current_returns_error() {
        assert!(MySqlAppRepository::validate_bed_state_transition("Unknown", "Available").is_err());
        assert!(MySqlAppRepository::validate_bed_state_transition("", "Available").is_err());
    }

    #[test]
    fn mask_mrn_short_value_returns_four_stars() {
        assert_eq!(MySqlAppRepository::mask_mrn("123"), "****");
        assert_eq!(MySqlAppRepository::mask_mrn("1234"), "****");
    }

    #[test]
    fn mask_mrn_long_value_exposes_last_four_chars() {
        assert_eq!(MySqlAppRepository::mask_mrn("MRN-12345678"), "***5678");
        assert_eq!(MySqlAppRepository::mask_mrn("ABCDEFGHIJ"), "***GHIJ");
    }

    #[test]
    fn mask_long_empty_value_returns_empty() {
        assert_eq!(MySqlAppRepository::mask_long(""), "");
        assert_eq!(MySqlAppRepository::mask_long("   "), "");
    }

    #[test]
    fn mask_long_non_empty_returns_redacted_sentinel() {
        let result = MySqlAppRepository::mask_long("some clinical data");
        assert_eq!(result, "[REDACTED - privileged reveal required]");
    }

    #[test]
    fn to_masked_patient_masks_sensitive_fields() {
        use crate::repositories::app_repository::PatientSensitiveRecord;
        let row = PatientSensitiveRecord {
            id: 1,
            mrn: "MRN-12345678".to_string(),
            first_name: "Jane".to_string(),
            last_name: "Doe".to_string(),
            birth_date: "1990-05-15".to_string(),
            gender: "F".to_string(),
            phone: "555-0000".to_string(),
            email: "jane@example.com".to_string(),
            allergies: "peanuts".to_string(),
            contraindications: "aspirin".to_string(),
            history: "hypertension".to_string(),
        };
        let dto = MySqlAppRepository::to_masked_patient(row);
        assert_eq!(dto.mrn, "***5678");
        assert_eq!(dto.allergies, "[REDACTED - privileged reveal required]");
        assert_eq!(dto.contraindications, "[REDACTED - privileged reveal required]");
        assert_eq!(dto.history, "[REDACTED - privileged reveal required]");
    }

    #[test]
    fn to_masked_patient_preserves_non_sensitive_fields() {
        use crate::repositories::app_repository::PatientSensitiveRecord;
        let row = PatientSensitiveRecord {
            id: 7,
            mrn: "MRN-00009999".to_string(),
            first_name: "John".to_string(),
            last_name: "Smith".to_string(),
            birth_date: "1985-03-20".to_string(),
            gender: "M".to_string(),
            phone: "555-1234".to_string(),
            email: "john@example.com".to_string(),
            allergies: "none".to_string(),
            contraindications: "none".to_string(),
            history: "none".to_string(),
        };
        let dto = MySqlAppRepository::to_masked_patient(row);
        assert_eq!(dto.id, 7);
        assert_eq!(dto.first_name, "John");
        assert_eq!(dto.last_name, "Smith");
        assert_eq!(dto.birth_date, "1985-03-20");
        assert_eq!(dto.gender, "M");
        assert_eq!(dto.phone, "555-1234");
        assert_eq!(dto.email, "john@example.com");
    }

    #[test]
    fn to_masked_patient_empty_sensitive_fields_are_empty_not_redacted() {
        use crate::repositories::app_repository::PatientSensitiveRecord;
        let row = PatientSensitiveRecord {
            id: 2,
            mrn: "12".to_string(),
            first_name: "A".to_string(),
            last_name: "B".to_string(),
            birth_date: "2000-01-01".to_string(),
            gender: "M".to_string(),
            phone: "".to_string(),
            email: "".to_string(),
            allergies: "".to_string(),
            contraindications: "   ".to_string(),
            history: "".to_string(),
        };
        let dto = MySqlAppRepository::to_masked_patient(row);
        assert_eq!(dto.allergies, "");
        assert_eq!(dto.contraindications, "");
        assert_eq!(dto.history, "");
        assert_eq!(dto.mrn, "****");
    }
}
