use contracts::{AttachmentMetadataDto, BedDto, BedEventDto, PatientProfileDto, PatientSearchResultDto, RevisionTimelineDto};

use crate::contracts::ApiError;
use crate::infrastructure::security::field_crypto::FieldCrypto;
use crate::repositories::app_repository::{AttachmentStorageRecord, BedTransitionDbRequest, PatientSensitiveRecord};
use super::MySqlAppRepository;

impl MySqlAppRepository {
    pub(super) async fn create_patient_impl(
        &self,
        created_by: i64,
        mrn: &str,
        first_name: &str,
        last_name: &str,
        birth_date: &str,
        gender: &str,
        phone: &str,
        email: &str,
        allergies: &str,
        contraindications: &str,
        history: &str,
    ) -> Result<i64, ApiError> {
        let mrn_cipher = self.field_crypto.encrypt(mrn)?;
        let allergies_cipher = self.field_crypto.encrypt(allergies)?;
        let contraindications_cipher = self.field_crypto.encrypt(contraindications)?;
        let history_cipher = self.field_crypto.encrypt(history)?;
        let mrn_hash = FieldCrypto::hash_for_lookup(mrn);
        let mrn_masked_unique = format!("MASKED-{}", &mrn_hash[..16]);

        let result = sqlx::query(
            "INSERT INTO patients
             (mrn, first_name, last_name, birth_date, gender, phone, email, allergies, contraindications, history,
              mrn_cipher, mrn_hash, allergies_cipher, contraindications_cipher, history_cipher, encryption_key_version,
              created_by, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NOW(), NOW())",
        )
        .bind(mrn_masked_unique)
        .bind(first_name)
        .bind(last_name)
        .bind(birth_date)
        .bind(gender)
        .bind(phone)
        .bind(email)
        .bind("[MASKED]")
        .bind("[MASKED]")
        .bind("[MASKED]")
        .bind(mrn_cipher)
        .bind(mrn_hash)
        .bind(allergies_cipher)
        .bind(contraindications_cipher)
        .bind(history_cipher)
        .bind(self.field_crypto.active_key_version())
        .bind(created_by)
        .execute(&self.pool)
        .await?;
        let patient_id = result.last_insert_id() as i64;

        sqlx::query(
            "INSERT INTO patient_assignments (patient_id, user_id, assignment_type, assigned_by, assigned_at)
             VALUES (?, ?, 'owner', ?, NOW())
             ON DUPLICATE KEY UPDATE assignment_type = VALUES(assignment_type)",
        )
        .bind(patient_id)
        .bind(created_by)
        .bind(created_by)
        .execute(&self.pool)
        .await?;

        Ok(patient_id)
    }

    pub(super) async fn can_access_patient_impl(&self, user_id: i64, role_name: &str, patient_id: i64) -> Result<bool, ApiError> {
        if self.has_global_patient_access(role_name).await? {
            let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(1) FROM patients WHERE id = ?")
                .bind(patient_id)
                .fetch_one(&self.pool)
                .await?;
            return Ok(exists > 0);
        }

        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(1)
             FROM patient_assignments pa
             JOIN patients p ON p.id = pa.patient_id
             WHERE pa.patient_id = ? AND pa.user_id = ?",
        )
        .bind(patient_id)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(count > 0)
    }

    pub(super) async fn assign_patient_impl(&self, patient_id: i64, target_user_id: i64, assignment_type: &str, assigned_by: i64) -> Result<(), ApiError> {
        sqlx::query(
            "INSERT INTO patient_assignments (patient_id, user_id, assignment_type, assigned_by, assigned_at)
             VALUES (?, ?, ?, ?, NOW())
             ON DUPLICATE KEY UPDATE assignment_type = VALUES(assignment_type), assigned_by = VALUES(assigned_by), assigned_at = NOW()",
        )
        .bind(patient_id)
        .bind(target_user_id)
        .bind(assignment_type)
        .bind(assigned_by)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub(super) async fn get_patient_impl(&self, patient_id: i64) -> Result<Option<PatientProfileDto>, ApiError> {
        let row = self.get_patient_sensitive_impl(patient_id).await?;
        Ok(row.map(Self::to_masked_patient))
    }

    pub(super) async fn get_patient_sensitive_impl(&self, patient_id: i64) -> Result<Option<PatientSensitiveRecord>, ApiError> {
        let row = sqlx::query_as::<_, (i64, String, String, String, String, String, String, String, String, String, String, String, String, String, String)>(
            "SELECT id, mrn, first_name, last_name, birth_date, gender, phone, email,
                    COALESCE(mrn_cipher,''), COALESCE(allergies_cipher,''), COALESCE(contraindications_cipher,''), COALESCE(history_cipher,''),
                    allergies, contraindications, history
             FROM patients WHERE id = ?",
        )
        .bind(patient_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| {
            let mrn = if r.8.is_empty() { r.1 } else { self.field_crypto.decrypt(&r.8).unwrap_or_default() };
            let allergies = if r.9.is_empty() { r.12 } else { self.field_crypto.decrypt(&r.9).unwrap_or_default() };
            let contraindications = if r.10.is_empty() { r.13 } else { self.field_crypto.decrypt(&r.10).unwrap_or_default() };
            let history = if r.11.is_empty() { r.14 } else { self.field_crypto.decrypt(&r.11).unwrap_or_default() };
            PatientSensitiveRecord {
                id: r.0,
                mrn,
                first_name: r.2,
                last_name: r.3,
                birth_date: r.4,
                gender: r.5,
                phone: r.6,
                email: r.7,
                allergies,
                contraindications,
                history,
            }
        }))
    }

    pub(super) async fn list_patients_impl(&self, user_id: i64, role_name: &str, limit: i64, offset: i64) -> Result<Vec<PatientProfileDto>, ApiError> {
        let safe_limit = limit.clamp(1, 100);
        let safe_offset = offset.max(0);
        let rows = if self.has_global_patient_access(role_name).await? {
            sqlx::query_as::<_, (i64, String, String, String, String, String, String, String, String, String, String, String, String, String, String)>(
                "SELECT id, mrn, first_name, last_name, birth_date, gender, phone, email,
                        COALESCE(mrn_cipher,''), COALESCE(allergies_cipher,''), COALESCE(contraindications_cipher,''), COALESCE(history_cipher,''),
                        allergies, contraindications, history
                 FROM patients ORDER BY id DESC LIMIT ? OFFSET ?",
            )
            .bind(safe_limit)
            .bind(safe_offset)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, (i64, String, String, String, String, String, String, String, String, String, String, String, String, String, String)>(
                "SELECT p.id, p.mrn, p.first_name, p.last_name, p.birth_date, p.gender, p.phone, p.email,
                        COALESCE(p.mrn_cipher,''), COALESCE(p.allergies_cipher,''), COALESCE(p.contraindications_cipher,''), COALESCE(p.history_cipher,''),
                        p.allergies, p.contraindications, p.history
                 FROM patients p
                 JOIN patient_assignments pa ON pa.patient_id = p.id
                 WHERE pa.user_id = ?
                 ORDER BY p.id DESC LIMIT ? OFFSET ?",
            )
            .bind(user_id)
            .bind(safe_limit)
            .bind(safe_offset)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows
            .into_iter()
            .map(|r| {
                let mrn = if r.8.is_empty() { r.1 } else { self.field_crypto.decrypt(&r.8).unwrap_or_default() };
                let allergies = if r.9.is_empty() { r.12 } else { self.field_crypto.decrypt(&r.9).unwrap_or_default() };
                let contraindications = if r.10.is_empty() { r.13 } else { self.field_crypto.decrypt(&r.10).unwrap_or_default() };
                let history = if r.11.is_empty() { r.14 } else { self.field_crypto.decrypt(&r.11).unwrap_or_default() };
                Self::to_masked_patient(PatientSensitiveRecord {
                    id: r.0,
                    mrn,
                    first_name: r.2,
                    last_name: r.3,
                    birth_date: r.4,
                    gender: r.5,
                    phone: r.6,
                    email: r.7,
                    allergies,
                    contraindications,
                    history,
                })
            })
            .collect())
    }

    pub(super) async fn search_patients_impl(
        &self,
        user_id: i64,
        role_name: &str,
        query: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<PatientSearchResultDto>, ApiError> {
        let like = format!("%{}%", query);
        let hash = FieldCrypto::hash_for_lookup(query);
        let safe_limit = limit.clamp(1, 100);
        let safe_offset = offset.max(0);
        let rows = if self.has_global_patient_access(role_name).await? {
            sqlx::query_as::<_, (i64, String, String, String, String)>(
                "SELECT id, mrn, first_name, last_name, COALESCE(mrn_cipher,'')
                 FROM patients
                 WHERE mrn_hash = ? OR first_name LIKE ? OR last_name LIKE ?
                 ORDER BY id DESC LIMIT ? OFFSET ?",
            )
            .bind(&hash)
            .bind(&like)
            .bind(&like)
            .bind(safe_limit)
            .bind(safe_offset)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, (i64, String, String, String, String)>(
                "SELECT p.id, p.mrn, p.first_name, p.last_name, COALESCE(p.mrn_cipher,'')
                 FROM patients p
                 JOIN patient_assignments pa ON pa.patient_id = p.id
                 WHERE pa.user_id = ? AND (p.mrn_hash = ? OR p.first_name LIKE ? OR p.last_name LIKE ?)
                 ORDER BY p.id DESC LIMIT ? OFFSET ?",
            )
            .bind(user_id)
            .bind(&hash)
            .bind(&like)
            .bind(&like)
            .bind(safe_limit)
            .bind(safe_offset)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows
            .into_iter()
            .map(|(id, mrn, first, last, mrn_cipher)| {
                let clear_mrn = if mrn_cipher.is_empty() {
                    mrn
                } else {
                    self.field_crypto.decrypt(&mrn_cipher).unwrap_or_default()
                };
                PatientSearchResultDto {
                id,
                mrn: Self::mask_mrn(&clear_mrn),
                display_name: format!("{} {}", first, last),
                }
            })
            .collect())
    }

    pub(super) async fn update_patient_demographics_impl(
        &self,
        patient_id: i64,
        first_name: &str,
        last_name: &str,
        birth_date: &str,
        gender: &str,
        phone: &str,
        email: &str,
    ) -> Result<(), ApiError> {
        sqlx::query(
            "UPDATE patients
             SET first_name = ?, last_name = ?, birth_date = ?, gender = ?, phone = ?, email = ?, updated_at = NOW()
             WHERE id = ?",
        )
        .bind(first_name)
        .bind(last_name)
        .bind(birth_date)
        .bind(gender)
        .bind(phone)
        .bind(email)
        .bind(patient_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub(super) async fn update_patient_clinical_field_impl(&self, patient_id: i64, field_name: &str, value: &str) -> Result<(), ApiError> {
        let query = match field_name {
            "allergies" => "UPDATE patients SET allergies = ?, updated_at = NOW() WHERE id = ?",
            "contraindications" => "UPDATE patients SET contraindications = ?, updated_at = NOW() WHERE id = ?",
            "history" => "UPDATE patients SET history = ?, updated_at = NOW() WHERE id = ?",
            _ => return Err(ApiError::bad_request("Unsupported clinical field")),
        };

        sqlx::query(query)
            .bind("[MASKED]")
            .bind(patient_id)
            .execute(&self.pool)
            .await?;

        let cipher = self.field_crypto.encrypt(value)?;
        let encrypted_query = match field_name {
            "allergies" => "UPDATE patients SET allergies_cipher = ?, encryption_key_version = ? WHERE id = ?",
            "contraindications" => "UPDATE patients SET contraindications_cipher = ?, encryption_key_version = ? WHERE id = ?",
            "history" => "UPDATE patients SET history_cipher = ?, encryption_key_version = ? WHERE id = ?",
            _ => return Err(ApiError::bad_request("Unsupported clinical field")),
        };

        sqlx::query(encrypted_query)
            .bind(cipher)
            .bind(self.field_crypto.active_key_version())
            .bind(patient_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub(super) async fn create_patient_revision_impl(
        &self,
        patient_id: i64,
        entity_type: &str,
        before_json: &str,
        after_json: &str,
        reason: &str,
        actor_id: i64,
    ) -> Result<(), ApiError> {
        let before_cipher = self.field_crypto.encrypt(before_json)?;
        let after_cipher = self.field_crypto.encrypt(after_json)?;
        sqlx::query(
            "INSERT INTO patient_revisions (patient_id, entity_type, diff_before, diff_before_cipher, diff_after, diff_after_cipher, encryption_key_version, reason_for_change, actor_id, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NOW())",
        )
        .bind(patient_id)
        .bind(entity_type)
        .bind("[ENCRYPTED]")
        .bind(&before_cipher)
        .bind("[ENCRYPTED]")
        .bind(&after_cipher)
        .bind(self.field_crypto.active_key_version())
        .bind(reason)
        .bind(actor_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub(super) async fn get_patient_revisions_impl(&self, patient_id: i64) -> Result<Vec<RevisionTimelineDto>, ApiError> {
        let rows = sqlx::query_as::<_, (i64, String, String, Option<String>, String, Option<String>, String, String, String)>(
            "SELECT pr.id, pr.entity_type, pr.diff_before, pr.diff_before_cipher, pr.diff_after, pr.diff_after_cipher, pr.reason_for_change, u.username, DATE_FORMAT(pr.created_at, '%Y-%m-%d %H:%i:%s')
             FROM patient_revisions pr
             JOIN users u ON u.id = pr.actor_id
             WHERE pr.patient_id = ?
             ORDER BY pr.id DESC",
        )
        .bind(patient_id)
        .fetch_all(&self.pool)
        .await?;

        let mut result = Vec::with_capacity(rows.len());
        for r in rows {
            // Prefer encrypted cipher columns; fall back to plaintext for pre-migration rows
            let diff_before = if let Some(ref cipher) = r.3 {
                self.field_crypto.decrypt(cipher).unwrap_or_else(|_| r.2.clone())
            } else {
                r.2
            };
            let diff_after = if let Some(ref cipher) = r.5 {
                self.field_crypto.decrypt(cipher).unwrap_or_else(|_| r.4.clone())
            } else {
                r.4
            };
            result.push(RevisionTimelineDto {
                id: r.0,
                entity_type: r.1,
                diff_before,
                diff_after,
                field_deltas_json: String::new(),
                reason_for_change: r.6,
                actor_username: r.7,
                created_at: r.8,
            });
        }
        Ok(result)
    }

    pub(super) async fn add_patient_visit_note_impl(&self, patient_id: i64, note: &str, actor_id: i64) -> Result<(), ApiError> {
        let note_cipher = self.field_crypto.encrypt(note)?;
        sqlx::query(
            "INSERT INTO patient_visit_notes (patient_id, note, note_cipher, encryption_key_version, created_by, created_at) VALUES (?, ?, ?, ?, ?, NOW())",
        )
        .bind(patient_id)
        .bind("[MASKED]")
        .bind(note_cipher)
        .bind(self.field_crypto.active_key_version())
        .bind(actor_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub(super) async fn list_attachments_impl(&self, patient_id: i64) -> Result<Vec<AttachmentMetadataDto>, ApiError> {
        let rows = sqlx::query_as::<_, (i64, String, String, i64, String, String)>(
            "SELECT pa.id, pa.file_name, pa.mime_type, pa.file_size_bytes, u.username, DATE_FORMAT(pa.uploaded_at, '%Y-%m-%d %H:%i:%s')
             FROM patient_attachments pa
             JOIN users u ON u.id = pa.uploaded_by
             WHERE pa.patient_id = ?
             ORDER BY pa.id DESC",
        )
        .bind(patient_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| AttachmentMetadataDto {
                id: r.0,
                file_name: r.1,
                mime_type: r.2,
                file_size_bytes: r.3,
                uploaded_by: r.4,
                uploaded_at: r.5,
            })
            .collect())
    }

    pub(super) async fn get_attachment_impl(
        &self,
        patient_id: i64,
        attachment_id: i64,
    ) -> Result<Option<AttachmentStorageRecord>, ApiError> {
        let row = sqlx::query_as::<_, (String, Vec<u8>)>(
            "SELECT mime_type, payload_blob
             FROM patient_attachments
             WHERE id = ? AND patient_id = ?",
        )
        .bind(attachment_id)
        .bind(patient_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(mime_type, payload_bytes)| AttachmentStorageRecord {
            mime_type,
            payload_bytes,
        }))
    }

    pub(super) async fn save_attachment_impl(
        &self,
        patient_id: i64,
        file_name: &str,
        mime_type: &str,
        file_size_bytes: i64,
        payload_bytes: &[u8],
        uploaded_by: i64,
    ) -> Result<(), ApiError> {
        sqlx::query(
            "INSERT INTO patient_attachments (patient_id, file_name, mime_type, file_size_bytes, payload_blob, uploaded_by, uploaded_at)
             VALUES (?, ?, ?, ?, ?, ?, NOW())",
        )
        .bind(patient_id)
        .bind(file_name)
        .bind(mime_type)
        .bind(file_size_bytes)
        .bind(payload_bytes)
        .bind(uploaded_by)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub(super) async fn list_beds_impl(&self) -> Result<Vec<BedDto>, ApiError> {
        let rows = sqlx::query_as::<_, (i64, String, String, String, String, String)>(
            "SELECT b.id, bl.name, u.name, r.code, b.bed_label, b.state
             FROM beds b
             JOIN rooms r ON r.id = b.room_id
             JOIN units u ON u.id = r.unit_id
             JOIN buildings bl ON bl.id = u.building_id
             ORDER BY b.id",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| BedDto {
                id: r.0,
                building: r.1,
                unit: r.2,
                room: r.3,
                bed_label: r.4,
                state: r.5,
            })
            .collect())
    }

    pub(super) async fn get_bed_state_impl(&self, bed_id: i64) -> Result<Option<String>, ApiError> {
        let state = sqlx::query_scalar::<_, String>("SELECT state FROM beds WHERE id = ?")
            .bind(bed_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(state)
    }

    pub(super) async fn apply_bed_transition_impl(
        &self,
        req: BedTransitionDbRequest,
    ) -> Result<(), ApiError> {
        // Validate-then-mutate inside ONE transaction. Every read uses
        // `FOR UPDATE` so concurrent writers are serialized on the affected
        // bed rows, and every prerequisite is verified BEFORE any UPDATE /
        // INSERT lands. If any check fails the whole transaction is dropped
        // and the underlying state is left untouched.
        let mut tx = self.pool.begin().await.map_err(|_| ApiError::Internal)?;

        let action = req.action.trim();
        let target_state = req.target_state.trim();
        let note = req.note.trim();

        // ── Step 1: lock and read source bed state ─────────────────────
        let current_state: Option<String> = sqlx::query_scalar(
            "SELECT state FROM beds WHERE id = ? FOR UPDATE",
        )
        .bind(req.bed_id)
        .fetch_optional(&mut *tx)
        .await?;
        let current_state = match current_state {
            Some(s) => s,
            None => return Err(ApiError::NotFound),
        };

        // ── Step 2: re-validate the state machine under the lock ───────
        Self::validate_bed_state_transition(&current_state, target_state)?;

        // ── Step 3: action-specific prerequisite checks ────────────────
        let active_occupant: Option<i64> = sqlx::query_scalar(
            "SELECT patient_id FROM bed_occupancies
             WHERE bed_id = ? AND checked_out_at IS NULL
             LIMIT 1 FOR UPDATE",
        )
        .bind(req.bed_id)
        .fetch_optional(&mut *tx)
        .await?;

        let mut target_locked_state: Option<String> = None;
        let mut target_active_occupant: Option<i64> = None;
        if let Some(target_bed) = req.related_bed_id {
            let s: Option<String> = sqlx::query_scalar(
                "SELECT state FROM beds WHERE id = ? FOR UPDATE",
            )
            .bind(target_bed)
            .fetch_optional(&mut *tx)
            .await?;
            match s {
                Some(s) => target_locked_state = Some(s),
                None => return Err(ApiError::NotFound),
            }
            target_active_occupant = sqlx::query_scalar(
                "SELECT patient_id FROM bed_occupancies
                 WHERE bed_id = ? AND checked_out_at IS NULL
                 LIMIT 1 FOR UPDATE",
            )
            .bind(target_bed)
            .fetch_optional(&mut *tx)
            .await?;
        }

        match action {
            "check-in" => {
                let pid = req.patient_id.ok_or_else(|| {
                    ApiError::bad_request("patient_id is required for check-in")
                })?;
                // Patient must exist; the SELECT FOR UPDATE keeps the row
                // pinned for the duration of the transaction so a concurrent
                // delete cannot strand an occupancy referencing a missing
                // patient.
                let exists: Option<i64> = sqlx::query_scalar(
                    "SELECT id FROM patients WHERE id = ? FOR UPDATE",
                )
                .bind(pid)
                .fetch_optional(&mut *tx)
                .await?;
                if exists.is_none() {
                    return Err(ApiError::bad_request(
                        "Patient referenced by check-in does not exist",
                    ));
                }
                if active_occupant.is_some() {
                    return Err(ApiError::bad_request(
                        "Bed already has an active occupant; check the current patient out first",
                    ));
                }
            }
            "check-out" => {
                if active_occupant.is_none() {
                    return Err(ApiError::bad_request(
                        "Bed has no active occupant to check out",
                    ));
                }
            }
            "transfer" => {
                let target_bed = req.related_bed_id.ok_or_else(|| {
                    ApiError::bad_request("related_bed_id is required for transfer")
                })?;
                if target_bed == req.bed_id {
                    return Err(ApiError::bad_request(
                        "transfer requires a distinct related_bed_id",
                    ));
                }
                let target_state_now = target_locked_state
                    .as_deref()
                    .ok_or(ApiError::Internal)?;
                Self::validate_bed_state_transition(target_state_now, "Occupied")?;
                if target_active_occupant.is_some() {
                    return Err(ApiError::bad_request(
                        "Target bed already has an active occupant",
                    ));
                }
                if active_occupant.is_none() {
                    return Err(ApiError::bad_request(
                        "Source bed has no active occupant to transfer",
                    ));
                }
            }
            "swap" => {
                let target_bed = req.related_bed_id.ok_or_else(|| {
                    ApiError::bad_request("related_bed_id is required for swap")
                })?;
                if target_bed == req.bed_id {
                    return Err(ApiError::bad_request(
                        "swap requires a distinct related_bed_id",
                    ));
                }
                let target_state_now = target_locked_state
                    .as_deref()
                    .ok_or(ApiError::Internal)?;
                if current_state != "Occupied" || target_state_now != "Occupied" {
                    return Err(ApiError::bad_request(
                        "Swap requires both beds to be Occupied",
                    ));
                }
                if active_occupant.is_none() || target_active_occupant.is_none() {
                    return Err(ApiError::bad_request(
                        "Swap requires both beds to have active occupants",
                    ));
                }
            }
            _ => {
                // Generic transition (no patient/occupancy mutation). The
                // state-machine validation above is sufficient.
            }
        }

        // ── Step 4: every prerequisite passed → apply mutations ────────
        match action {
            "check-in" => {
                let pid = req.patient_id.unwrap();
                sqlx::query("UPDATE beds SET state = ? WHERE id = ?")
                    .bind(target_state)
                    .bind(req.bed_id)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query(
                    "INSERT INTO bed_occupancies (bed_id, patient_id, checked_in_at)
                     VALUES (?, ?, NOW())",
                )
                .bind(req.bed_id)
                .bind(pid)
                .execute(&mut *tx)
                .await?;
            }
            "check-out" => {
                sqlx::query("UPDATE beds SET state = ? WHERE id = ?")
                    .bind(target_state)
                    .bind(req.bed_id)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query(
                    "UPDATE bed_occupancies SET checked_out_at = NOW(), checked_out_reason = ?
                     WHERE bed_id = ? AND checked_out_at IS NULL",
                )
                .bind("check-out")
                .bind(req.bed_id)
                .execute(&mut *tx)
                .await?;
            }
            "transfer" => {
                let target_bed = req.related_bed_id.unwrap();
                let occupant = active_occupant.unwrap();
                // Source: detach occupant, mark Cleaning.
                sqlx::query(
                    "UPDATE bed_occupancies SET checked_out_at = NOW(), checked_out_reason = ?
                     WHERE bed_id = ? AND checked_out_at IS NULL",
                )
                .bind("transfer")
                .bind(req.bed_id)
                .execute(&mut *tx)
                .await?;
                sqlx::query("UPDATE beds SET state = 'Cleaning' WHERE id = ?")
                    .bind(req.bed_id)
                    .execute(&mut *tx)
                    .await?;
                // Target: attach occupant, mark Occupied.
                sqlx::query(
                    "INSERT INTO bed_occupancies (bed_id, patient_id, checked_in_at)
                     VALUES (?, ?, NOW())",
                )
                .bind(target_bed)
                .bind(occupant)
                .execute(&mut *tx)
                .await?;
                sqlx::query("UPDATE beds SET state = 'Occupied' WHERE id = ?")
                    .bind(target_bed)
                    .execute(&mut *tx)
                    .await?;
            }
            "swap" => {
                let target_bed = req.related_bed_id.unwrap();
                let occupant_a = active_occupant.unwrap();
                let occupant_b = target_active_occupant.unwrap();
                sqlx::query(
                    "UPDATE bed_occupancies SET checked_out_at = NOW(), checked_out_reason = ?
                     WHERE bed_id IN (?, ?) AND checked_out_at IS NULL",
                )
                .bind("swap")
                .bind(req.bed_id)
                .bind(target_bed)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    "INSERT INTO bed_occupancies (bed_id, patient_id, checked_in_at)
                     VALUES (?, ?, NOW())",
                )
                .bind(target_bed)
                .bind(occupant_a)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    "INSERT INTO bed_occupancies (bed_id, patient_id, checked_in_at)
                     VALUES (?, ?, NOW())",
                )
                .bind(req.bed_id)
                .bind(occupant_b)
                .execute(&mut *tx)
                .await?;
                // Both beds remain Occupied.
            }
            _ => {
                sqlx::query("UPDATE beds SET state = ? WHERE id = ?")
                    .bind(target_state)
                    .bind(req.bed_id)
                    .execute(&mut *tx)
                    .await?;
            }
        }

        // ── Step 5: append the bed_event row in the same transaction ──
        let (event_to_state, event_patient): (String, Option<i64>) = match action {
            "transfer" => ("Occupied".to_string(), active_occupant.or(req.patient_id)),
            "swap" => ("Occupied".to_string(), active_occupant.or(req.patient_id)),
            "check-in" => (target_state.to_string(), req.patient_id),
            "check-out" => (target_state.to_string(), req.patient_id.or(active_occupant)),
            _ => (target_state.to_string(), req.patient_id),
        };
        sqlx::query(
            "INSERT INTO bed_events
             (action_type, from_bed_id, to_bed_id, from_state, to_state, patient_id, actor_id, note, occurred_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, NOW())",
        )
        .bind(action)
        .bind(req.bed_id)
        .bind(req.related_bed_id)
        .bind(&current_state)
        .bind(&event_to_state)
        .bind(event_patient)
        .bind(req.actor_id)
        .bind(note)
        .execute(&mut *tx)
        .await?;

        tx.commit().await.map_err(|_| ApiError::Internal)?;
        Ok(())
    }

    pub(super) async fn list_bed_events_impl(&self) -> Result<Vec<BedEventDto>, ApiError> {
        let rows = sqlx::query_as::<_, (i64, String, Option<i64>, Option<i64>, Option<String>, Option<String>, String, String)>(
            "SELECT be.id, be.action_type, be.from_bed_id, be.to_bed_id, be.from_state, be.to_state, u.username,
             DATE_FORMAT(be.occurred_at, '%Y-%m-%d %H:%i:%s')
             FROM bed_events be JOIN users u ON u.id = be.actor_id ORDER BY be.id DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| BedEventDto {
                id: r.0,
                action: r.1,
                from_bed_id: r.2,
                to_bed_id: r.3,
                from_state: r.4,
                to_state: r.5,
                actor_username: r.6,
                occurred_at: r.7,
            })
            .collect())
    }
}
