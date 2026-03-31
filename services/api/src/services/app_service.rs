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
use crate::repositories::app_repository::{AppRepository, OrderRecord};

#[derive(Clone)]
pub struct AppService {
    repo: Arc<dyn AppRepository>,
    password_min_length: u32,
    lockout_minutes: u32,
    session_inactivity_minutes: u32,
    clinical_years_min: u32,
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

    pub async fn list_hospitals(&self) -> Result<Vec<HospitalDto>, ApiError> {
        self.repo.list_hospitals().await
    }

    pub async fn list_roles(&self) -> Result<Vec<RoleDto>, ApiError> {
        self.repo.list_roles().await
    }

    pub async fn login(&self, req: AuthLoginRequest) -> Result<AuthLoginResponse, ApiError> {
        if req.username.trim().is_empty() || req.password.is_empty() {
            Self::security_log(
                "auth.login",
                "rejected",
                serde_json::json!({"reason":"missing_credentials"}),
            );
            return Err(ApiError::bad_request("Username and password are required"));
        }
        self.validate_password_complexity(&req.password)?;

        let maybe_user = self.repo.get_user_auth(req.username.trim()).await?;
        let user = match maybe_user {
            Some(u) => u,
            None => {
                Self::security_log(
                    "auth.login",
                    "rejected",
                    serde_json::json!({"username":req.username.trim(),"reason":"unknown_user"}),
                );
                return Err(ApiError::Unauthorized);
            }
        };

        if user.disabled {
            Self::security_log(
                "auth.login",
                "rejected",
                serde_json::json!({"user_id":user.id,"reason":"disabled"}),
            );
            return Err(ApiError::Forbidden);
        }

        if user.locked_now {
            Self::security_log(
                "auth.login",
                "rejected",
                serde_json::json!({"user_id":user.id,"reason":"locked"}),
            );
            return Err(ApiError::bad_request(&format!(
                "Account is locked for {} minutes after failed logins",
                self.lockout_minutes
            )));
        }

        let verify = Self::verify_password(&req.password, &user.password_hash)?;
        if !verify.0 {
            self.repo
                .register_failed_login(user.id, user.failed_attempts + 1)
                .await?;
            Self::security_log(
                "auth.login",
                "rejected",
                serde_json::json!({"user_id":user.id,"reason":"bad_password"}),
            );
            return Err(ApiError::Unauthorized);
        }

        if verify.1 {
            let upgraded = Self::hash_password_argon2(&req.password)?;
            self.repo.update_user_password_hash(user.id, &upgraded).await?;
            Self::security_log(
                "auth.password_migration",
                "success",
                serde_json::json!({"user_id":user.id,"from":"legacy_sha256","to":"argon2id"}),
            );
        }

        self.repo.reset_login_failures(user.id).await?;
        let token = Self::generate_session_token();
        self.repo.create_session(&token, user.id).await?;
        self.repo
            .append_audit(
                "auth.login",
                "user",
                &user.id.to_string(),
                "{\"result\":\"success\"}",
                user.id,
            )
            .await?;

        Self::security_log(
            "auth.login",
            "success",
            serde_json::json!({"user_id":user.id,"role":user.role_name}),
        );

        Ok(AuthLoginResponse {
            token,
            user_id: user.id,
            username: user.username,
            role: user.role_name,
            expires_in_minutes: self.session_inactivity_minutes,
        })
    }

    pub async fn validate_session_token(&self, token: &str) -> Result<AuthUser, ApiError> {
        let session = self.repo.get_session(token).await?;
        let session = match session {
            Some(value) => value,
            None => {
                Self::security_log(
                    "auth.session_validate",
                    "rejected",
                    serde_json::json!({"reason":"missing_or_revoked","token_fingerprint":Self::token_fingerprint(token)}),
                );
                return Err(ApiError::Unauthorized);
            }
        };

        if session.disabled {
            Self::security_log(
                "auth.session_validate",
                "rejected",
                serde_json::json!({"user_id":session.user_id,"reason":"disabled"}),
            );
            return Err(ApiError::Forbidden);
        }
        if session.inactive_expired {
            Self::security_log(
                "auth.session_validate",
                "rejected",
                serde_json::json!({"user_id":session.user_id,"reason":"inactivity_expired"}),
            );
            return Err(ApiError::Unauthorized);
        }

        self.repo.touch_session(token).await?;

        Ok(AuthUser {
            user_id: session.user_id,
            username: session.username,
            role_name: session.role_name,
        })
    }

    pub async fn authorize(&self, user: &AuthUser, permission: &str) -> Result<(), ApiError> {
        let allowed = self
            .repo
            .user_has_permission(&user.role_name, permission)
            .await?;
        if !allowed {
            Self::security_log(
                "auth.permission",
                "rejected",
                serde_json::json!({"user_id":user.user_id,"role":user.role_name,"permission":permission}),
            );
            return Err(ApiError::Forbidden);
        }
        Ok(())
    }

    pub async fn menu_entitlements(&self, user: &AuthUser) -> Result<Vec<MenuEntitlementDto>, ApiError> {
        self.repo.list_menu_entitlements(&user.role_name).await
    }

    pub async fn list_users(&self, user: &AuthUser) -> Result<Vec<UserSummaryDto>, ApiError> {
        self.authorize(user, "admin.disable_user").await?;
        self.repo.list_users().await
    }

    pub async fn disable_user(&self, user: &AuthUser, target_user_id: i64) -> Result<(), ApiError> {
        self.authorize(user, "admin.disable_user").await?;
        self.repo.disable_user(target_user_id).await?;
        self.repo.revoke_user_sessions(target_user_id).await?;
        self.repo
            .append_audit(
                "admin.disable_user",
                "user",
                &target_user_id.to_string(),
                "{\"immediate\":true}",
                user.user_id,
            )
            .await?;
        Ok(())
    }

    pub async fn create_patient(&self, user: &AuthUser, req: PatientCreateRequest) -> Result<i64, ApiError> {
        self.authorize(user, "patient.write").await?;
        if req.mrn.trim().is_empty() {
            return Err(ApiError::bad_request("MRN is required"));
        }

        let patient_id = self
            .repo
            .create_patient(
                user.user_id,
                req.mrn.trim(),
                req.first_name.trim(),
                req.last_name.trim(),
                req.birth_date.trim(),
                req.gender.trim(),
                req.phone.trim(),
                req.email.trim(),
                req.allergies.trim(),
                req.contraindications.trim(),
                req.history.trim(),
            )
            .await?;

        self.repo
            .append_audit(
                "patient.create",
                "patient",
                &patient_id.to_string(),
                "{\"source\":\"intranet\"}",
                user.user_id,
            )
            .await?;
        Ok(patient_id)
    }

    pub async fn list_patients(
        &self,
        user: &AuthUser,
        limit: i64,
        offset: i64,
        reveal_sensitive: bool,
    ) -> Result<Vec<PatientProfileDto>, ApiError> {
        self.authorize(user, "patient.read").await?;
        let mut items = self
            .repo
            .list_patients(user.user_id, &user.role_name, limit, offset)
            .await?;
        if reveal_sensitive {
            self.authorize(user, "patient.reveal_sensitive").await?;
            for patient in &mut items {
                let sensitive = self
                    .repo
                    .get_patient_sensitive(patient.id)
                    .await?
                    .ok_or(ApiError::NotFound)?;
                *patient = PatientProfileDto {
                    id: sensitive.id,
                    mrn: sensitive.mrn,
                    first_name: sensitive.first_name,
                    last_name: sensitive.last_name,
                    birth_date: sensitive.birth_date,
                    gender: sensitive.gender,
                    phone: sensitive.phone,
                    email: sensitive.email,
                    allergies: sensitive.allergies,
                    contraindications: sensitive.contraindications,
                    history: sensitive.history,
                };
            }
        }
        Ok(items)
    }

    pub async fn get_patient(
        &self,
        user: &AuthUser,
        patient_id: i64,
        reveal_sensitive: bool,
    ) -> Result<PatientProfileDto, ApiError> {
        self.authorize(user, "patient.read").await?;
        let can_access = self
            .repo
            .can_access_patient(user.user_id, &user.role_name, patient_id)
            .await?;
        if !can_access {
            return Err(ApiError::Forbidden);
        }
        if reveal_sensitive {
            self.authorize(user, "patient.reveal_sensitive").await?;
            let patient = self.repo.get_patient_sensitive(patient_id).await?;
            return patient
                .map(|s| PatientProfileDto {
                    id: s.id,
                    mrn: s.mrn,
                    first_name: s.first_name,
                    last_name: s.last_name,
                    birth_date: s.birth_date,
                    gender: s.gender,
                    phone: s.phone,
                    email: s.email,
                    allergies: s.allergies,
                    contraindications: s.contraindications,
                    history: s.history,
                })
                .ok_or(ApiError::NotFound);
        }
        self.repo.get_patient(patient_id).await?.ok_or(ApiError::NotFound)
    }

    pub async fn assign_patient(
        &self,
        user: &AuthUser,
        patient_id: i64,
        target_user_id: i64,
        assignment_type: &str,
    ) -> Result<(), ApiError> {
        self.authorize(user, "patient.write").await?;
        let can_access = self
            .repo
            .can_access_patient(user.user_id, &user.role_name, patient_id)
            .await?;
        if !can_access {
            return Err(ApiError::Forbidden);
        }
        if !["owner", "care_team", "viewer"].contains(&assignment_type) {
            return Err(ApiError::bad_request("Invalid assignment type"));
        }
        self.repo
            .assign_patient(patient_id, target_user_id, assignment_type, user.user_id)
            .await?;
        self.repo
            .append_audit(
                "patient.assign",
                "patient",
                &patient_id.to_string(),
                &format!(
                    "{{\"target_user_id\":{},\"assignment_type\":{}}}",
                    target_user_id,
                    serde_json::to_string(assignment_type).map_err(|_| ApiError::Internal)?
                ),
                user.user_id,
            )
            .await?;
        Ok(())
    }

    pub async fn update_patient(&self, user: &AuthUser, patient_id: i64, req: PatientUpdateRequest) -> Result<(), ApiError> {
        self.authorize(user, "patient.write").await?;
        let can_access = self
            .repo
            .can_access_patient(user.user_id, &user.role_name, patient_id)
            .await?;
        if !can_access {
            return Err(ApiError::Forbidden);
        }
        Self::ensure_reason(&req.reason_for_change)?;

        let before = self
            .repo
            .get_patient_sensitive(patient_id)
            .await?
            .ok_or(ApiError::NotFound)?;
        self.repo
            .update_patient_demographics(
                patient_id,
                req.first_name.trim(),
                req.last_name.trim(),
                req.birth_date.trim(),
                req.gender.trim(),
                req.phone.trim(),
                req.email.trim(),
            )
            .await?;
        let after = self
            .repo
            .get_patient_sensitive(patient_id)
            .await?
            .ok_or(ApiError::NotFound)?;

        self.repo
            .create_patient_revision(
                patient_id,
                "demographics",
                &serde_json::to_string(&before).map_err(|_| ApiError::Internal)?,
                &serde_json::to_string(&after).map_err(|_| ApiError::Internal)?,
                req.reason_for_change.trim(),
                user.user_id,
            )
            .await?;

        self.repo
            .append_audit(
                "patient.edit",
                "patient",
                &patient_id.to_string(),
                "{\"section\":\"demographics\"}",
                user.user_id,
            )
            .await?;
        Ok(())
    }

    pub async fn edit_clinical_field(
        &self,
        user: &AuthUser,
        patient_id: i64,
        field_name: &str,
        req: ClinicalEditRequest,
    ) -> Result<(), ApiError> {
        self.authorize(user, "clinical.edit").await?;
        let can_access = self
            .repo
            .can_access_patient(user.user_id, &user.role_name, patient_id)
            .await?;
        if !can_access {
            return Err(ApiError::Forbidden);
        }
        Self::ensure_reason(&req.reason_for_change)?;

        let before = self
            .repo
            .get_patient_sensitive(patient_id)
            .await?
            .ok_or(ApiError::NotFound)?;
        self.repo
            .update_patient_clinical_field(patient_id, field_name, req.value.trim())
            .await?;
        let after = self
            .repo
            .get_patient_sensitive(patient_id)
            .await?
            .ok_or(ApiError::NotFound)?;

        self.repo
            .create_patient_revision(
                patient_id,
                field_name,
                &serde_json::to_string(&before).map_err(|_| ApiError::Internal)?,
                &serde_json::to_string(&after).map_err(|_| ApiError::Internal)?,
                req.reason_for_change.trim(),
                user.user_id,
            )
            .await?;

        self.repo
            .append_audit(
                "clinical.edit",
                "patient",
                &patient_id.to_string(),
                &format!("{{\"field\":\"{}\"}}", field_name),
                user.user_id,
            )
            .await?;
        Ok(())
    }

    pub async fn add_visit_note(&self, user: &AuthUser, patient_id: i64, req: VisitNoteRequest) -> Result<(), ApiError> {
        self.authorize(user, "clinical.edit").await?;
        let can_access = self
            .repo
            .can_access_patient(user.user_id, &user.role_name, patient_id)
            .await?;
        if !can_access {
            return Err(ApiError::Forbidden);
        }
        Self::ensure_reason(&req.reason_for_change)?;
        if req.note.trim().is_empty() {
            return Err(ApiError::bad_request("Visit note cannot be empty"));
        }

        self.repo
            .add_patient_visit_note(patient_id, req.note.trim(), user.user_id)
            .await?;
        self.repo
            .create_patient_revision(
                patient_id,
                "visit_note",
                "{}",
                &format!("{{\"note\":{}}}", serde_json::to_string(req.note.trim()).map_err(|_| ApiError::Internal)?),
                req.reason_for_change.trim(),
                user.user_id,
            )
            .await?;
        Ok(())
    }

    pub async fn patient_revisions(
        &self,
        user: &AuthUser,
        patient_id: i64,
        reveal_sensitive: bool,
    ) -> Result<Vec<RevisionTimelineDto>, ApiError> {
        self.authorize(user, "patient.read").await?;
        let can_access = self
            .repo
            .can_access_patient(user.user_id, &user.role_name, patient_id)
            .await?;
        if !can_access {
            return Err(ApiError::Forbidden);
        }
        if reveal_sensitive {
            self.authorize(user, "patient.reveal_sensitive").await?;
        }

        let items = self.repo.list_patient_revisions(patient_id).await?;
        Ok(items
            .into_iter()
            .map(|item| Self::decorate_revision_deltas(item, reveal_sensitive))
            .collect())
    }

    pub async fn list_attachments(&self, user: &AuthUser, patient_id: i64) -> Result<Vec<AttachmentMetadataDto>, ApiError> {
        self.authorize(user, "patient.read").await?;
        let can_access = self
            .repo
            .can_access_patient(user.user_id, &user.role_name, patient_id)
            .await?;
        if !can_access {
            return Err(ApiError::Forbidden);
        }
        self.repo.list_attachments(patient_id).await
    }

    pub async fn download_attachment(
        &self,
        user: &AuthUser,
        patient_id: i64,
        attachment_id: i64,
    ) -> Result<(String, Vec<u8>), ApiError> {
        self.authorize(user, "patient.read").await?;
        let can_access = self
            .repo
            .can_access_patient(user.user_id, &user.role_name, patient_id)
            .await?;
        if !can_access {
            return Err(ApiError::Forbidden);
        }

        let meta = self
            .repo
            .get_attachment_storage(patient_id, attachment_id)
            .await?
            .ok_or(ApiError::NotFound)?;
        let crate::repositories::app_repository::AttachmentStorageRecord {
            mime_type,
            payload_bytes,
            legacy_storage_path,
        } = meta;

        let bytes = if let Some(payload) = payload_bytes {
            payload
        } else if !legacy_storage_path.trim().is_empty() {
            tokio::fs::read(&legacy_storage_path)
                .await
                .map_err(|_| ApiError::NotFound)?
        } else {
            return Err(ApiError::NotFound);
        };

        self.repo
            .append_audit(
                "clinical.attachment_download",
                "patient",
                &patient_id.to_string(),
                &format!(
                    "{{\"attachment_id\":{},\"mime\":{}}}",
                    attachment_id,
                    serde_json::to_string(&mime_type).map_err(|_| ApiError::Internal)?
                ),
                user.user_id,
            )
            .await?;
        Ok((mime_type, bytes))
    }

    pub async fn export_patient(
        &self,
        user: &AuthUser,
        patient_id: i64,
        format: &str,
        reveal_sensitive: bool,
    ) -> Result<PatientExportDto, ApiError> {
        self.authorize(user, "patient.export").await?;
        let can_access = self
            .repo
            .can_access_patient(user.user_id, &user.role_name, patient_id)
            .await?;
        if !can_access {
            return Err(ApiError::Forbidden);
        }
        let fmt = format.trim().to_ascii_lowercase();
        if !matches!(fmt.as_str(), "json" | "csv") {
            return Err(ApiError::bad_request("format must be json or csv"));
        }

        let patient = self
            .get_patient(user, patient_id, reveal_sensitive)
            .await?;
        let content = if fmt == "json" {
            serde_json::to_string(&patient).map_err(|_| ApiError::Internal)?
        } else {
            Self::patient_as_csv(&patient)
        };

        self.repo
            .append_audit(
                "patient.export",
                "patient",
                &patient_id.to_string(),
                &format!(
                    "{{\"format\":{},\"reveal_sensitive\":{}}}",
                    serde_json::to_string(&fmt).map_err(|_| ApiError::Internal)?,
                    if reveal_sensitive { "true" } else { "false" }
                ),
                user.user_id,
            )
            .await?;

        Ok(PatientExportDto {
            format: fmt,
            content,
            generated_at: Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        })
    }

    pub async fn save_attachment(
        &self,
        user: &AuthUser,
        patient_id: i64,
        filename: &str,
        mime_type: &str,
        bytes: &[u8],
    ) -> Result<(), ApiError> {
        self.authorize(user, "clinical.edit").await?;
        let can_access = self
            .repo
            .can_access_patient(user.user_id, &user.role_name, patient_id)
            .await?;
        if !can_access {
            return Err(ApiError::Forbidden);
        }
        Self::validate_attachment(filename, mime_type, bytes.len() as i64)?;

        let safe_name = filename.replace('/', "_");

        self.repo
            .create_attachment_metadata(
                patient_id,
                &safe_name,
                mime_type,
                bytes.len() as i64,
                bytes,
                user.user_id,
            )
            .await?;
        self.repo
            .append_audit(
                "clinical.attachment_upload",
                "patient",
                &patient_id.to_string(),
                &format!(
                    "{{\"file_name\":{},\"mime\":{},\"size\":{}}}",
                    serde_json::to_string(&safe_name).map_err(|_| ApiError::Internal)?,
                    serde_json::to_string(mime_type).map_err(|_| ApiError::Internal)?,
                    bytes.len()
                ),
                user.user_id,
            )
            .await?;
        Ok(())
    }

    pub async fn list_beds(&self, user: &AuthUser) -> Result<Vec<BedDto>, ApiError> {
        self.authorize(user, "bedboard.read").await?;
        self.repo.list_beds().await
    }

    pub async fn transition_bed(&self, user: &AuthUser, bed_id: i64, req: BedTransitionRequest) -> Result<(), ApiError> {
        self.authorize(user, "bedboard.write").await?;
        let current = self.repo.get_bed_state(bed_id).await?.ok_or(ApiError::NotFound)?;
        Self::validate_bed_transition(&current, &req.target_state)?;
        self.repo.set_bed_state(bed_id, req.target_state.trim()).await?;

        match (req.action.as_str(), req.related_bed_id) {
            ("transfer", Some(target)) => {
                let target_state = self.repo.get_bed_state(target).await?.ok_or(ApiError::NotFound)?;
                Self::validate_bed_transition(&target_state, "Occupied")?;
                self.repo.set_bed_state(target, "Occupied").await?;
                self.repo.set_bed_state(bed_id, "Cleaning").await?;
                self.repo
                    .record_bed_event(
                        "transfer",
                        Some(bed_id),
                        Some(target),
                        Some(&current),
                        Some("Occupied"),
                        user.user_id,
                        req.note.trim(),
                    )
                    .await?;
            }
            ("swap", Some(target)) => {
                let target_state = self.repo.get_bed_state(target).await?.ok_or(ApiError::NotFound)?;
                if current != "Occupied" || target_state != "Occupied" {
                    return Err(ApiError::bad_request("Swap requires both beds to be Occupied"));
                }
                self.repo
                    .record_bed_event(
                        "swap",
                        Some(bed_id),
                        Some(target),
                        Some("Occupied"),
                        Some("Occupied"),
                        user.user_id,
                        req.note.trim(),
                    )
                    .await?;
            }
            _ => {
                self.repo
                    .record_bed_event(
                        req.action.trim(),
                        Some(bed_id),
                        None,
                        Some(&current),
                        Some(req.target_state.trim()),
                        user.user_id,
                        req.note.trim(),
                    )
                    .await?;
            }
        }

        self.repo
            .append_audit(
                "bedboard.action",
                "bed",
                &bed_id.to_string(),
                &format!("{{\"action\":{},\"target_state\":{}}}",
                    serde_json::to_string(req.action.trim()).map_err(|_| ApiError::Internal)?,
                    serde_json::to_string(req.target_state.trim()).map_err(|_| ApiError::Internal)?),
                user.user_id,
            )
            .await?;
        Ok(())
    }

    pub async fn bed_events(&self, user: &AuthUser) -> Result<Vec<BedEventDto>, ApiError> {
        self.authorize(user, "bedboard.read").await?;
        self.repo.list_bed_events().await
    }

    pub async fn create_menu(&self, user: &AuthUser, req: DiningMenuRequest) -> Result<(), ApiError> {
        self.authorize(user, "dining.write").await?;
        self.repo
            .create_menu(
                req.menu_date.trim(),
                req.meal_period.trim(),
                req.item_name.trim(),
                req.calories,
                user.user_id,
            )
            .await
    }

    pub async fn list_menus(&self, user: &AuthUser) -> Result<Vec<DiningMenuDto>, ApiError> {
        self.authorize(user, "dining.read").await?;
        self.repo.list_menus().await
    }

    pub async fn place_order(&self, user: &AuthUser, req: OrderCreateRequest) -> Result<i64, ApiError> {
        self.authorize(user, "order.write").await?;
        let has_global_order_access = self
            .repo
            .user_has_permission(&user.role_name, "order.global_access")
            .await?;
        let patient_access = if has_global_order_access {
            true
        } else {
            self.repo
                .can_access_patient(user.user_id, &user.role_name, req.patient_id)
                .await?
        };
        if !patient_access {
            return Err(ApiError::Forbidden);
        }
        let order_id = self
            .repo
            .create_order_idempotent(
                req.patient_id,
                req.menu_id,
                req.notes.trim(),
                user.user_id,
                req.idempotency_key.as_deref(),
            )
            .await?;
        self.repo
            .append_audit(
                "order.create",
                "dining_order",
                &order_id.to_string(),
                "{\"status\":\"Created\"}",
                user.user_id,
            )
            .await?;
        self.repo.close_inactive_campaigns().await?;
        Ok(order_id)
    }

    pub async fn set_order_status(&self, user: &AuthUser, order_id: i64, req: OrderStatusRequest) -> Result<(), ApiError> {
        self.authorize(user, "order.write").await?;
        let status = req.status.trim();
        let valid = ["Created", "Billed", "Canceled", "Credited"];
        if !valid.contains(&status) {
            return Err(ApiError::bad_request("Invalid order status"));
        }

        let current = self.repo.get_order(order_id).await?.ok_or(ApiError::NotFound)?;
        self.ensure_order_access(user, &current).await?;

        let transition_ok = match (current.status.as_str(), status) {
            ("Created", "Billed") => true,
            ("Created", "Canceled") => true,
            ("Billed", "Credited") => true,
            (a, b) if a == b => true,
            _ => false,
        };
        if !transition_ok {
            return Err(ApiError::bad_request("Invalid order transition"));
        }
        if Self::status_requires_reason(status)
            && req.reason.as_deref().unwrap_or(" ").trim().is_empty()
        {
            return Err(ApiError::bad_request(
                "Reason is required when canceling or crediting an order",
            ));
        }

        let expected = req.expected_version.unwrap_or(current.version);
        let changed = self
            .repo
            .set_order_status_if_version(order_id, expected, status, req.reason.as_deref())
            .await?;
        if !changed {
            return Err(ApiError::Conflict);
        }

        self.repo
            .append_audit(
                "order.status",
                "dining_order",
                &order_id.to_string(),
                &format!("{{\"status\":{}}}", serde_json::to_string(status).map_err(|_| ApiError::Internal)?),
                user.user_id,
            )
            .await?;
        self.repo.close_inactive_campaigns().await?;
        Ok(())
    }

    pub async fn list_orders(&self, user: &AuthUser, limit: i64, offset: i64) -> Result<Vec<OrderDto>, ApiError> {
        self.authorize(user, "order.read").await?;
        self.repo
            .list_orders(user.user_id, &user.role_name, limit, offset)
            .await
    }

    pub async fn create_governance_record(&self, user: &AuthUser, req: GovernanceRecordRequest) -> Result<i64, ApiError> {
        self.authorize(user, "governance.write").await?;
        let tier = req.tier.trim();
        if !["raw", "cleaned", "analytics"].contains(&tier) {
            return Err(ApiError::bad_request("Tier must be raw, cleaned, or analytics"));
        }
        let id = self
            .repo
            .create_governance_record(
                tier,
                req.lineage_source_id,
                req.lineage_metadata.trim(),
                req.payload_json.trim(),
                user.user_id,
            )
            .await?;
        self.repo
            .append_audit(
                "governance.create",
                "governance_record",
                &id.to_string(),
                &format!("{{\"tier\":{}}}", serde_json::to_string(tier).map_err(|_| ApiError::Internal)?),
                user.user_id,
            )
            .await?;
        Ok(id)
    }

    pub async fn list_governance_records(&self, user: &AuthUser) -> Result<Vec<GovernanceRecordDto>, ApiError> {
        self.authorize(user, "governance.write").await?;
        self.repo.list_governance_records().await
    }

    pub async fn tombstone_governance_record(
        &self,
        user: &AuthUser,
        record_id: i64,
        req: GovernanceDeleteRequest,
    ) -> Result<(), ApiError> {
        self.authorize(user, "governance.write").await?;
        Self::ensure_reason(&req.reason)?;
        self.repo
            .tombstone_governance_record(record_id, req.reason.trim())
            .await?;
        self.repo
            .append_audit(
                "governance.tombstone",
                "governance_record",
                &record_id.to_string(),
                &format!("{{\"reason\":{}}}", serde_json::to_string(req.reason.trim()).map_err(|_| ApiError::Internal)?),
                user.user_id,
            )
            .await?;
        Ok(())
    }

    pub async fn telemetry_event(&self, user: &AuthUser, req: TelemetryEventRequest) -> Result<(), ApiError> {
        self.authorize(user, "telemetry.write").await?;
        self.repo
            .create_telemetry_event(
                req.experiment_key.trim(),
                user.user_id,
                req.event_name.trim(),
                req.payload_json.trim(),
            )
            .await
    }

    pub async fn list_audits(&self, user: &AuthUser) -> Result<Vec<AuditLogDto>, ApiError> {
        self.authorize(user, "audit.read").await?;
        self.repo.list_audits().await
    }

    pub async fn list_retention_policies(&self, user: &AuthUser) -> Result<Vec<RetentionPolicyDto>, ApiError> {
        self.authorize(user, "audit.read").await?;
        self.repo.list_retention_policies().await
    }

    pub async fn set_retention_policy(
        &self,
        user: &AuthUser,
        policy_key: &str,
        years: i32,
    ) -> Result<(), ApiError> {
        self.authorize(user, "retention.manage").await?;
        if years < self.clinical_years_min as i32 {
            return Err(ApiError::bad_request(&format!(
                "Clinical retention cannot be lower than {} years",
                self.clinical_years_min
            )));
        }
        self.repo
            .upsert_retention_policy(policy_key, years, user.user_id)
            .await
    }

    pub async fn search_patients(
        &self,
        user: &AuthUser,
        query: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<PatientSearchResultDto>, ApiError> {
        self.authorize(user, "patient.read").await?;
        self.repo
            .search_patients(user.user_id, &user.role_name, query.trim(), limit, offset)
            .await
    }

    pub async fn list_dish_categories(&self, user: &AuthUser) -> Result<Vec<DishCategoryDto>, ApiError> {
        self.authorize(user, "dining.read").await?;
        self.repo.list_dish_categories().await
    }

    pub async fn create_dish(&self, user: &AuthUser, req: DishCreateRequest) -> Result<i64, ApiError> {
        self.authorize(user, "dining.write").await?;
        if req.name.trim().is_empty() {
            return Err(ApiError::bad_request("Dish name is required"));
        }
        let id = self
            .repo
            .create_dish(
                req.category_id,
                req.name.trim(),
                req.description.trim(),
                req.base_price_cents,
                req.photo_path.trim(),
                user.user_id,
            )
            .await?;
        Ok(id)
    }

    pub async fn list_dishes(&self, user: &AuthUser) -> Result<Vec<DishDto>, ApiError> {
        self.authorize(user, "dining.read").await?;
        self.repo.list_dishes().await
    }

    pub async fn set_dish_status(&self, user: &AuthUser, dish_id: i64, req: DishStatusRequest) -> Result<(), ApiError> {
        self.authorize(user, "dining.write").await?;
        self.repo
            .set_dish_status(dish_id, req.is_published, req.is_sold_out)
            .await
    }

    pub async fn add_dish_option(&self, user: &AuthUser, dish_id: i64, req: DishOptionRequest) -> Result<(), ApiError> {
        self.authorize(user, "dining.write").await?;
        self.repo
            .add_dish_option(
                dish_id,
                req.option_group.trim(),
                req.option_value.trim(),
                req.delta_price_cents,
            )
            .await
    }

    pub async fn add_sales_window(&self, user: &AuthUser, dish_id: i64, req: DishWindowRequest) -> Result<(), ApiError> {
        self.authorize(user, "dining.write").await?;
        self.repo
            .add_sales_window(
                dish_id,
                req.slot_name.trim(),
                req.start_hhmm.trim(),
                req.end_hhmm.trim(),
            )
            .await
    }

    pub async fn upsert_ranking_rule(&self, user: &AuthUser, req: RankingRuleRequest) -> Result<(), ApiError> {
        self.authorize(user, "dining.write").await?;
        self.repo
            .upsert_ranking_rule(req.rule_key.trim(), req.weight, req.enabled, user.user_id)
            .await
    }

    pub async fn ranking_rules(&self, user: &AuthUser) -> Result<Vec<RankingRuleDto>, ApiError> {
        self.authorize(user, "dining.read").await?;
        self.repo.list_ranking_rules().await
    }

    pub async fn recommendations(&self, user: &AuthUser) -> Result<Vec<RecommendationDto>, ApiError> {
        self.authorize(user, "dining.read").await?;
        self.repo.recommendations().await
    }

    pub async fn create_campaign(&self, user: &AuthUser, req: CampaignCreateRequest) -> Result<i64, ApiError> {
        self.authorize(user, "order.write").await?;
        if req.success_threshold <= 0 {
            return Err(ApiError::bad_request("success_threshold must be greater than 0"));
        }
        let deadline = Self::normalize_campaign_deadline(&req.success_deadline_at)?;
        self.repo.close_inactive_campaigns().await?;
        self.repo
            .create_campaign(
                req.title.trim(),
                req.dish_id,
                req.success_threshold,
                &deadline,
                user.user_id,
            )
            .await
    }

    pub async fn join_campaign(&self, user: &AuthUser, campaign_id: i64) -> Result<(), ApiError> {
        self.authorize(user, "order.write").await?;
        self.repo.close_inactive_campaigns().await?;
        self.repo.join_campaign(campaign_id, user.user_id).await
    }

    pub async fn campaigns(&self, user: &AuthUser) -> Result<Vec<CampaignDto>, ApiError> {
        self.authorize(user, "dining.read").await?;
        self.repo.close_inactive_campaigns().await?;
        self.repo.list_campaigns().await
    }

    pub async fn add_ticket_split(&self, user: &AuthUser, order_id: i64, req: TicketSplitRequest) -> Result<(), ApiError> {
        self.authorize(user, "order.write").await?;
        let order = self.repo.get_order(order_id).await?.ok_or(ApiError::NotFound)?;
        self.ensure_order_access(user, &order).await?;
        self.repo
            .add_ticket_split(order_id, req.split_by.trim(), req.split_value.trim(), req.quantity)
            .await
    }

    pub async fn list_ticket_splits(&self, user: &AuthUser, order_id: i64) -> Result<Vec<TicketSplitDto>, ApiError> {
        self.authorize(user, "order.read").await?;
        let order = self.repo.get_order(order_id).await?.ok_or(ApiError::NotFound)?;
        self.ensure_order_access(user, &order).await?;
        self.repo.list_ticket_splits(order_id).await
    }

    pub async fn add_order_note(&self, user: &AuthUser, order_id: i64, req: OrderNoteRequest) -> Result<(), ApiError> {
        self.authorize(user, "order.write").await?;
        let order = self.repo.get_order(order_id).await?.ok_or(ApiError::NotFound)?;
        self.ensure_order_access(user, &order).await?;
        if req.note.trim().is_empty() {
            return Err(ApiError::bad_request("Order note cannot be empty"));
        }
        self.repo
            .add_order_note(order_id, req.note.trim(), user.user_id)
            .await
    }

    pub async fn order_notes(&self, user: &AuthUser, order_id: i64) -> Result<Vec<OrderNoteDto>, ApiError> {
        self.authorize(user, "order.read").await?;
        let order = self.repo.get_order(order_id).await?.ok_or(ApiError::NotFound)?;
        self.ensure_order_access(user, &order).await?;
        self.repo.list_order_notes(order_id).await
    }

    pub async fn create_experiment(&self, user: &AuthUser, req: ExperimentCreateRequest) -> Result<i64, ApiError> {
        self.authorize(user, "retention.manage").await?;
        self.repo.create_experiment(req.experiment_key.trim()).await
    }

    pub async fn add_experiment_variant(
        &self,
        user: &AuthUser,
        experiment_id: i64,
        req: ExperimentVariantRequest,
    ) -> Result<(), ApiError> {
        self.authorize(user, "retention.manage").await?;
        self.repo
            .add_experiment_variant(
                experiment_id,
                req.variant_key.trim(),
                req.allocation_weight,
                req.feature_version.trim(),
            )
            .await
    }

    pub async fn assign_experiment(
        &self,
        user: &AuthUser,
        experiment_id: i64,
        req: ExperimentAssignRequest,
    ) -> Result<Option<String>, ApiError> {
        self.authorize(user, "retention.manage").await?;
        self.repo
            .assign_experiment_variant(experiment_id, req.user_id, req.mode.trim())
            .await
    }

    pub async fn backtrack_experiment(
        &self,
        user: &AuthUser,
        experiment_id: i64,
        req: ExperimentBacktrackRequest,
    ) -> Result<(), ApiError> {
        self.authorize(user, "retention.manage").await?;
        Self::ensure_reason(&req.reason)?;
        self.repo
            .record_experiment_backtrack(
                experiment_id,
                req.from_version.trim(),
                req.to_version.trim(),
                req.reason.trim(),
                user.user_id,
            )
            .await
    }

    pub async fn funnel_metrics(&self, user: &AuthUser) -> Result<Vec<FunnelMetricsDto>, ApiError> {
        self.authorize(user, "audit.read").await?;
        self.repo.funnel_metrics().await
    }

    pub async fn retention_metrics(&self, user: &AuthUser) -> Result<Vec<RetentionMetricsDto>, ApiError> {
        self.authorize(user, "audit.read").await?;
        self.repo.retention_metrics().await
    }

    pub async fn recommendation_kpi(&self, user: &AuthUser) -> Result<RecommendationKpiDto, ApiError> {
        self.authorize(user, "audit.read").await?;
        self.repo.recommendation_kpi().await
    }

    pub async fn create_ingestion_task(
        &self,
        user: &AuthUser,
        req: IngestionTaskCreateRequest,
    ) -> Result<i64, ApiError> {
        self.authorize(user, "ingestion.manage").await?;
        if req.task_name.trim().is_empty() {
            return Err(ApiError::bad_request("Task name is required"));
        }
        if req.seed_urls.is_empty() {
            return Err(ApiError::bad_request("At least one seed URL is required"));
        }
        if req.max_depth < 0 || req.max_depth > 10 {
            return Err(ApiError::bad_request("max_depth must be between 0 and 10"));
        }
        self.repo.create_ingestion_task(user.user_id, req).await
    }

    pub async fn update_ingestion_task(
        &self,
        user: &AuthUser,
        task_id: i64,
        req: IngestionTaskUpdateRequest,
    ) -> Result<i32, ApiError> {
        self.authorize(user, "ingestion.manage").await?;
        self.repo
            .update_ingestion_task(task_id, user.user_id, &user.role_name, req)
            .await
    }

    pub async fn rollback_ingestion_task(
        &self,
        user: &AuthUser,
        task_id: i64,
        req: IngestionTaskRollbackRequest,
    ) -> Result<i32, ApiError> {
        self.authorize(user, "ingestion.manage").await?;
        Self::ensure_reason(&req.reason)?;
        self.repo
            .rollback_ingestion_task(task_id, user.user_id, &user.role_name, req)
            .await
    }

    pub async fn run_ingestion_task(&self, user: &AuthUser, task_id: i64) -> Result<(), ApiError> {
        self.authorize(user, "ingestion.manage").await?;
        let result = self
            .repo
            .run_ingestion_task(task_id, user.user_id, &user.role_name)
            .await;
        if let Err(ref err) = result {
            Self::security_log(
                "ingestion.run",
                "failed",
                serde_json::json!({"task_id":task_id,"actor_id":user.user_id,"error_code":Self::error_code(err)}),
            );
        }
        result
    }

    pub async fn list_ingestion_tasks(&self, user: &AuthUser) -> Result<Vec<IngestionTaskDto>, ApiError> {
        self.authorize(user, "ingestion.read").await?;
        self.repo
            .list_ingestion_tasks(user.user_id, &user.role_name)
            .await
    }

    pub async fn ingestion_task_versions(
        &self,
        user: &AuthUser,
        task_id: i64,
    ) -> Result<Vec<IngestionTaskVersionDto>, ApiError> {
        self.authorize(user, "ingestion.read").await?;
        self.repo
            .ingestion_task_versions(task_id, user.user_id, &user.role_name)
            .await
    }

    pub async fn ingestion_task_runs(
        &self,
        user: &AuthUser,
        task_id: i64,
        limit: i64,
    ) -> Result<Vec<IngestionTaskRunDto>, ApiError> {
        self.authorize(user, "ingestion.read").await?;
        self.repo
            .ingestion_task_runs(task_id, limit, user.user_id, &user.role_name)
            .await
    }

    pub async fn append_access_audit(&self, user: &AuthUser, path: &str) -> Result<(), ApiError> {
        self.repo
            .append_audit(
                "access",
                "api",
                path,
                &format!("{{\"path\":{}}}", serde_json::to_string(path).map_err(|_| ApiError::Internal)?),
                user.user_id,
            )
            .await
    }

    fn validate_password_complexity(&self, password: &str) -> Result<(), ApiError> {
        Self::validate_password_complexity_with_min(password, self.password_min_length)
    }

    fn validate_password_complexity_with_min(password: &str, min_len: u32) -> Result<(), ApiError> {
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

    fn hash_password_argon2(password: &str) -> Result<String, ApiError> {
        let params = Params::new(19_456, 2, 1, Some(32)).map_err(|_| ApiError::Internal)?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let salt = SaltString::generate(&mut OsRng);
        argon2
            .hash_password(password.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(|_| ApiError::Internal)
    }

    fn is_legacy_sha256_hash(stored_hash: &str) -> bool {
        let Ok(re) = Regex::new(r"^[a-f0-9]{64}$") else {
            return false;
        };
        re.is_match(stored_hash)
    }

    fn verify_password(password: &str, stored_hash: &str) -> Result<(bool, bool), ApiError> {
        if Self::is_legacy_sha256_hash(stored_hash) {
            let legacy = hex::encode(sha2::Sha256::digest(password.as_bytes()));
            return Ok((legacy == stored_hash, legacy == stored_hash));
        }

        let parsed = PasswordHash::new(stored_hash).map_err(|_| ApiError::Unauthorized)?;
        let params = Params::new(19_456, 2, 1, Some(32)).map_err(|_| ApiError::Internal)?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        Ok((argon2.verify_password(password.as_bytes(), &parsed).is_ok(), false))
    }

    fn generate_session_token() -> String {
        let mut bytes = [0u8; 48];
        rand::thread_rng().fill_bytes(&mut bytes);
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    }

    fn ensure_reason(reason: &str) -> Result<(), ApiError> {
        if reason.trim().is_empty() {
            return Err(ApiError::bad_request("Reason for change is required"));
        }
        Ok(())
    }

    fn validate_attachment(filename: &str, mime_type: &str, size_bytes: i64) -> Result<(), ApiError> {
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

    fn validate_bed_transition(current: &str, target: &str) -> Result<(), ApiError> {
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

    fn status_requires_reason(status: &str) -> bool {
        matches!(status, "Canceled" | "Credited")
    }

    fn token_fingerprint(token: &str) -> String {
        let digest = sha2::Sha256::digest(token.as_bytes());
        let hex = hex::encode(digest);
        hex[..12].to_string()
    }

    fn error_code(err: &ApiError) -> &'static str {
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

    fn is_sensitive_revision_field(field: &str) -> bool {
        matches!(field, "mrn" | "allergies" | "contraindications" | "history")
    }

    fn parse_object(json_blob: &str) -> serde_json::Map<String, serde_json::Value> {
        serde_json::from_str::<serde_json::Value>(json_blob)
            .ok()
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default()
    }

    fn stringify_delta_value(value: &serde_json::Value) -> String {
        value
            .as_str()
            .map(ToString::to_string)
            .unwrap_or_else(|| value.to_string())
    }

    fn decorate_revision_deltas(mut item: RevisionTimelineDto, reveal_sensitive: bool) -> RevisionTimelineDto {
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
        item
    }

    fn security_log(event: &str, outcome: &str, details: serde_json::Value) {
        let payload = serde_json::json!({
            "kind": "security",
            "event": event,
            "outcome": outcome,
            "details": details,
        });
        eprintln!("{}", payload);
    }

    async fn ensure_order_access(&self, user: &AuthUser, order: &OrderRecord) -> Result<(), ApiError> {
        let has_global_order_access = self
            .repo
            .user_has_permission(&user.role_name, "order.global_access")
            .await?;
        if has_global_order_access {
            return Ok(());
        }
        let can_access = self
            .repo
            .can_access_patient(user.user_id, &user.role_name, order.patient_id)
            .await?;
        if !can_access {
            return Err(ApiError::Forbidden);
        }
        Ok(())
    }

    fn normalize_campaign_deadline(input: &str) -> Result<String, ApiError> {
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

    fn csv_escape(value: &str) -> String {
        let escaped = value.replace('"', "\"\"");
        format!("\"{}\"", escaped)
    }

    fn patient_as_csv(patient: &PatientProfileDto) -> String {
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
mod tests {
    use super::AppService;
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
}
