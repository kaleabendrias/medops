use contracts::{
    AttachmentMetadataDto, BedDto, BedEventDto, BedTransitionRequest, ClinicalEditRequest,
    PatientCreateRequest, PatientExportDto, PatientProfileDto, PatientSearchResultDto,
    PatientUpdateRequest, RevisionTimelineDto, VisitNoteRequest,
};

use crate::contracts::{ApiError, AuthUser};
use crate::repositories::app_repository::BedTransitionDbRequest;
use super::AppService;

impl AppService {
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
            return Err(ApiError::NotFound);
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
            return Err(ApiError::NotFound);
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
            return Err(ApiError::NotFound);
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
            return Err(ApiError::NotFound);
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
            return Err(ApiError::NotFound);
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
            return Err(ApiError::NotFound);
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
            return Err(ApiError::NotFound);
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
            return Err(ApiError::NotFound);
        }

        let meta = self
            .repo
            .get_attachment_storage(patient_id, attachment_id)
            .await?
            .ok_or(ApiError::NotFound)?;
        let crate::repositories::app_repository::AttachmentStorageRecord {
            mime_type,
            payload_bytes,
        } = meta;

        let bytes = payload_bytes;

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
            return Err(ApiError::NotFound);
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
            generated_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
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
            return Err(ApiError::NotFound);
        }
        Self::validate_attachment(filename, mime_type, bytes.len() as i64)?;
        Self::verify_content_signature(bytes, mime_type)?;

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

    pub async fn list_beds(&self, user: &AuthUser) -> Result<Vec<BedDto>, ApiError> {
        self.authorize(user, "bedboard.read").await?;
        self.repo.list_beds().await
    }

    pub async fn transition_bed(&self, user: &AuthUser, bed_id: i64, req: BedTransitionRequest) -> Result<(), ApiError> {
        self.authorize(user, "bedboard.write").await?;

        // Cheap, stateless pre-flight: catch obviously-malformed transitions
        // (unknown current state, illegal target) without paying for a DB
        // round-trip. The repository re-validates everything inside the
        // transaction, so this is purely a UX optimisation — it CANNOT be
        // relied on for correctness.
        if let Some(current) = self.repo.get_bed_state(bed_id).await? {
            Self::validate_bed_transition(&current, &req.target_state)?;
        } else {
            return Err(ApiError::NotFound);
        }

        // Hand the request off to the atomic repository entry point, which
        // re-locks the affected rows, re-validates the state machine,
        // verifies action-specific prerequisites (patient existence,
        // occupancy invariants, target-bed eligibility for transfer/swap),
        // and applies every mutation inside ONE transaction. A failure at
        // any step rolls the whole transaction back, so the caller can
        // never observe a partially-applied transition.
        self.repo
            .apply_bed_transition(BedTransitionDbRequest {
                bed_id,
                action: req.action.clone(),
                target_state: req.target_state.clone(),
                related_bed_id: req.related_bed_id,
                patient_id: req.patient_id,
                note: req.note.clone(),
                actor_id: user.user_id,
            })
            .await?;

        self.repo
            .append_audit(
                "bedboard.action",
                "bed",
                &bed_id.to_string(),
                &format!("{{\"action\":{},\"target_state\":{},\"patient_id\":{}}}",
                    serde_json::to_string(req.action.trim()).map_err(|_| ApiError::Internal)?,
                    serde_json::to_string(req.target_state.trim()).map_err(|_| ApiError::Internal)?,
                    req.patient_id.map(|p| p.to_string()).unwrap_or_else(|| "null".to_string())),
                user.user_id,
            )
            .await?;
        Ok(())
    }

    pub async fn bed_events(&self, user: &AuthUser) -> Result<Vec<BedEventDto>, ApiError> {
        self.authorize(user, "bedboard.read").await?;
        self.repo.list_bed_events().await
    }
}
