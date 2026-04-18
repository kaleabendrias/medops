use async_trait::async_trait;
use contracts::{
    AttachmentMetadataDto, AuditLogDto, BedDto, BedEventDto, CampaignDto, DiningMenuDto,
    DishCategoryDto, DishDto, FunnelMetricsDto, GovernanceRecordDto, HospitalDto,
    IngestionTaskCreateRequest, IngestionTaskDto, IngestionTaskRollbackRequest,
    IngestionTaskRunDto, IngestionTaskUpdateRequest, IngestionTaskVersionDto,
    MenuEntitlementDto, OrderDto, OrderNoteDto, PatientProfileDto, PatientSearchResultDto,
    RankingRuleDto, RecommendationKpiDto, RecommendationDto, RetentionMetricsDto,
    RetentionPolicyDto, RevisionTimelineDto, RoleDto, TicketSplitDto, UserSummaryDto,
};

use crate::contracts::ApiError;
use crate::repositories::app_repository::{
    AppRepository, AttachmentStorageRecord, BedTransitionDbRequest, OrderRecord,
    PatientSensitiveRecord, SessionRecord, UserAuthRecord,
};
use super::MySqlAppRepository;

#[async_trait]
impl AppRepository for MySqlAppRepository {
    async fn list_hospitals(&self) -> Result<Vec<HospitalDto>, ApiError> {
        let rows = sqlx::query_as::<_, (i64, String, String, String, String, String)>(
            "SELECT id, code, name, city, country, status FROM hospitals ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, code, name, city, country, status)| HospitalDto {
                id,
                code,
                name,
                city,
                country,
                status,
            })
            .collect())
    }

    async fn list_roles(&self) -> Result<Vec<RoleDto>, ApiError> {
        let rows = sqlx::query_as::<_, (i64, String, String)>(
            "SELECT id, name, description FROM roles ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, name, description)| RoleDto {
                id,
                name,
                description,
            })
            .collect())
    }

    async fn get_user_auth(&self, username: &str) -> Result<Option<UserAuthRecord>, ApiError> {
        let row = sqlx::query_as::<_, (i64, String, String, String, bool, i32, i32)>(
            "SELECT u.id, u.username, u.password_hash, r.name, u.is_disabled, u.failed_attempts,
             CASE WHEN u.locked_until IS NOT NULL AND u.locked_until > NOW() THEN 1 ELSE 0 END
             FROM users u JOIN roles r ON r.id = u.role_id WHERE u.username = ?",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(
            |(id, username, password_hash, role_name, disabled, failed_attempts, locked)| {
                UserAuthRecord {
                    id,
                    username,
                    password_hash,
                    role_name,
                    disabled,
                    failed_attempts,
                    locked_now: locked == 1,
                }
            },
        ))
    }

    async fn register_failed_login(&self, user_id: i64, attempt_count: i32) -> Result<(), ApiError> {
        let should_lock = attempt_count >= self.lockout_failed_attempts;
        sqlx::query(
            "UPDATE users
             SET failed_attempts = ?,
                 locked_until = CASE WHEN ? THEN DATE_ADD(NOW(), INTERVAL ? MINUTE) ELSE locked_until END,
                 updated_at = NOW()
             WHERE id = ?",
        )
        .bind(attempt_count)
        .bind(should_lock)
        .bind(self.lockout_minutes)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn update_user_password_hash(&self, user_id: i64, new_hash: &str) -> Result<(), ApiError> {
        sqlx::query("UPDATE users SET password_hash = ?, updated_at = NOW() WHERE id = ?")
            .bind(new_hash)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn reset_login_failures(&self, user_id: i64) -> Result<(), ApiError> {
        sqlx::query(
            "UPDATE users SET failed_attempts = 0, locked_until = NULL, last_activity_at = NOW(), updated_at = NOW() WHERE id = ?",
        )
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn create_session(&self, token: &str, user_id: i64) -> Result<(), ApiError> {
        // Persist only the SHA-256 digest of the bearer token. The raw token
        // is returned to the caller in-process and never stored.
        let token_hash = Self::hash_session_token(token);
        sqlx::query(
            "INSERT INTO sessions (session_token_hash, user_id, created_at, last_activity_at, revoked_at)
             VALUES (?, ?, NOW(), NOW(), NULL)",
        )
        .bind(token_hash)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_session(&self, token: &str) -> Result<Option<SessionRecord>, ApiError> {
        // Hash the inbound bearer token before comparing against the column,
        // so the raw token never appears in the SQL parameter set.
        let token_hash = Self::hash_session_token(token);
        let row = sqlx::query_as::<_, (i64, String, String, bool, i32)>(
            "SELECT u.id, u.username, r.name, u.is_disabled,
             CASE WHEN TIMESTAMPDIFF(MINUTE, s.last_activity_at, NOW()) >= ? THEN 1 ELSE 0 END AS inactive_expired
             FROM sessions s
             JOIN users u ON u.id = s.user_id
             JOIN roles r ON r.id = u.role_id
             WHERE s.session_token_hash = ? AND s.revoked_at IS NULL",
        )
        .bind(self.session_inactivity_minutes)
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(
            |(user_id, username, role_name, disabled, inactive_expired)| SessionRecord {
                user_id,
                username,
                role_name,
                disabled,
                inactive_expired: inactive_expired == 1,
            },
        ))
    }

    async fn touch_session(&self, token: &str) -> Result<(), ApiError> {
        let token_hash = Self::hash_session_token(token);
        sqlx::query("UPDATE sessions SET last_activity_at = NOW() WHERE session_token_hash = ? AND revoked_at IS NULL")
            .bind(token_hash)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn delete_session(&self, token: &str) -> Result<(), ApiError> {
        let token_hash = Self::hash_session_token(token);
        sqlx::query("UPDATE sessions SET revoked_at = NOW() WHERE session_token_hash = ? AND revoked_at IS NULL")
            .bind(token_hash)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn revoke_user_sessions(&self, user_id: i64) -> Result<(), ApiError> {
        sqlx::query("UPDATE sessions SET revoked_at = NOW() WHERE user_id = ? AND revoked_at IS NULL")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn user_has_permission(&self, role_name: &str, permission_key: &str) -> Result<bool, ApiError> {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(1)
             FROM role_permissions rp
             JOIN roles r ON r.id = rp.role_id
             WHERE r.name = ? AND rp.permission_key = ?",
        )
        .bind(role_name)
        .bind(permission_key)
        .fetch_one(&self.pool)
        .await?;
        Ok(count > 0)
    }

    async fn list_menu_entitlements(&self, role_name: &str) -> Result<Vec<MenuEntitlementDto>, ApiError> {
        let rows = sqlx::query_as::<_, (String, bool)>(
            "SELECT me.menu_key, me.allowed
             FROM menu_entitlements me
             JOIN roles r ON r.id = me.role_id
             WHERE r.name = ?
             ORDER BY me.menu_key",
        )
        .bind(role_name)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(menu_key, allowed)| MenuEntitlementDto { menu_key, allowed })
            .collect())
    }

    async fn disable_user(&self, user_id: i64) -> Result<(), ApiError> {
        sqlx::query("UPDATE users SET is_disabled = TRUE, updated_at = NOW() WHERE id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn list_users(&self) -> Result<Vec<UserSummaryDto>, ApiError> {
        let rows = sqlx::query_as::<_, (i64, String, String, bool)>(
            "SELECT u.id, u.username, r.name, u.is_disabled
             FROM users u JOIN roles r ON r.id = u.role_id ORDER BY u.id",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, username, role, disabled)| UserSummaryDto {
                id,
                username,
                role,
                disabled,
            })
            .collect())
    }

    async fn create_patient(
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
        self.create_patient_impl(created_by, mrn, first_name, last_name, birth_date, gender, phone, email, allergies, contraindications, history).await
    }

    async fn can_access_patient(&self, user_id: i64, role_name: &str, patient_id: i64) -> Result<bool, ApiError> {
        self.can_access_patient_impl(user_id, role_name, patient_id).await
    }

    async fn assign_patient(&self, patient_id: i64, target_user_id: i64, assignment_type: &str, assigned_by: i64) -> Result<(), ApiError> {
        self.assign_patient_impl(patient_id, target_user_id, assignment_type, assigned_by).await
    }

    async fn get_patient(&self, patient_id: i64) -> Result<Option<PatientProfileDto>, ApiError> {
        self.get_patient_impl(patient_id).await
    }

    async fn get_patient_sensitive(&self, patient_id: i64) -> Result<Option<PatientSensitiveRecord>, ApiError> {
        self.get_patient_sensitive_impl(patient_id).await
    }

    async fn list_patients(&self, user_id: i64, role_name: &str, limit: i64, offset: i64) -> Result<Vec<PatientProfileDto>, ApiError> {
        self.list_patients_impl(user_id, role_name, limit, offset).await
    }

    async fn search_patients(&self, user_id: i64, role_name: &str, query: &str, limit: i64, offset: i64) -> Result<Vec<PatientSearchResultDto>, ApiError> {
        self.search_patients_impl(user_id, role_name, query, limit, offset).await
    }

    async fn update_patient_demographics(
        &self,
        patient_id: i64,
        first_name: &str,
        last_name: &str,
        birth_date: &str,
        gender: &str,
        phone: &str,
        email: &str,
    ) -> Result<(), ApiError> {
        self.update_patient_demographics_impl(patient_id, first_name, last_name, birth_date, gender, phone, email).await
    }

    async fn update_patient_clinical_field(&self, patient_id: i64, field_name: &str, value: &str) -> Result<(), ApiError> {
        self.update_patient_clinical_field_impl(patient_id, field_name, value).await
    }

    async fn create_patient_revision(
        &self,
        patient_id: i64,
        entity_type: &str,
        before_json: &str,
        after_json: &str,
        reason: &str,
        actor_id: i64,
    ) -> Result<(), ApiError> {
        self.create_patient_revision_impl(patient_id, entity_type, before_json, after_json, reason, actor_id).await
    }

    async fn list_patient_revisions(&self, patient_id: i64) -> Result<Vec<RevisionTimelineDto>, ApiError> {
        self.get_patient_revisions_impl(patient_id).await
    }

    async fn add_patient_visit_note(&self, patient_id: i64, note: &str, actor_id: i64) -> Result<(), ApiError> {
        self.add_patient_visit_note_impl(patient_id, note, actor_id).await
    }

    async fn list_attachments(&self, patient_id: i64) -> Result<Vec<AttachmentMetadataDto>, ApiError> {
        self.list_attachments_impl(patient_id).await
    }

    async fn get_attachment_storage(&self, patient_id: i64, attachment_id: i64) -> Result<Option<AttachmentStorageRecord>, ApiError> {
        self.get_attachment_impl(patient_id, attachment_id).await
    }

    async fn create_attachment_metadata(
        &self,
        patient_id: i64,
        file_name: &str,
        mime_type: &str,
        file_size_bytes: i64,
        payload_bytes: &[u8],
        uploaded_by: i64,
    ) -> Result<(), ApiError> {
        self.save_attachment_impl(patient_id, file_name, mime_type, file_size_bytes, payload_bytes, uploaded_by).await
    }

    async fn list_beds(&self) -> Result<Vec<BedDto>, ApiError> {
        self.list_beds_impl().await
    }

    async fn get_bed_state(&self, bed_id: i64) -> Result<Option<String>, ApiError> {
        self.get_bed_state_impl(bed_id).await
    }

    async fn set_bed_state(&self, bed_id: i64, state: &str) -> Result<(), ApiError> {
        sqlx::query("UPDATE beds SET state = ? WHERE id = ?")
            .bind(state)
            .bind(bed_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn record_bed_event(
        &self,
        action: &str,
        from_bed_id: Option<i64>,
        to_bed_id: Option<i64>,
        from_state: Option<&str>,
        to_state: Option<&str>,
        actor_id: i64,
        note: &str,
        patient_id: Option<i64>,
    ) -> Result<(), ApiError> {
        sqlx::query(
            "INSERT INTO bed_events (action_type, from_bed_id, to_bed_id, from_state, to_state, patient_id, actor_id, note, occurred_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, NOW())",
        )
        .bind(action)
        .bind(from_bed_id)
        .bind(to_bed_id)
        .bind(from_state)
        .bind(to_state)
        .bind(patient_id)
        .bind(actor_id)
        .bind(note)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn check_in_patient(&self, bed_id: i64, patient_id: i64) -> Result<(), ApiError> {
        sqlx::query(
            "INSERT INTO bed_occupancies (bed_id, patient_id, checked_in_at) VALUES (?, ?, NOW())",
        )
        .bind(bed_id)
        .bind(patient_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn check_out_patient(&self, bed_id: i64, reason: &str) -> Result<(), ApiError> {
        sqlx::query(
            "UPDATE bed_occupancies SET checked_out_at = NOW(), checked_out_reason = ?
             WHERE bed_id = ? AND checked_out_at IS NULL",
        )
        .bind(reason)
        .bind(bed_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn active_bed_occupant(&self, bed_id: i64) -> Result<Option<i64>, ApiError> {
        let row = sqlx::query_scalar::<_, i64>(
            "SELECT patient_id FROM bed_occupancies WHERE bed_id = ? AND checked_out_at IS NULL LIMIT 1",
        )
        .bind(bed_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    async fn apply_bed_transition(&self, req: BedTransitionDbRequest) -> Result<(), ApiError> {
        self.apply_bed_transition_impl(req).await
    }

    async fn list_bed_events(&self) -> Result<Vec<BedEventDto>, ApiError> {
        self.list_bed_events_impl().await
    }

    async fn create_menu(&self, menu_date: &str, meal_period: &str, item_name: &str, calories: i32, actor_id: i64) -> Result<(), ApiError> {
        self.create_menu_impl(menu_date, meal_period, item_name, calories, actor_id).await
    }

    async fn list_menus(&self) -> Result<Vec<DiningMenuDto>, ApiError> {
        self.list_menus_impl().await
    }

    async fn validate_menu_orderable(&self, menu_id: i64) -> Result<(), ApiError> {
        self.validate_menu_orderable_impl(menu_id).await
    }

    async fn create_order(&self, patient_id: i64, menu_id: i64, notes: &str, actor_id: i64) -> Result<i64, ApiError> {
        self.create_order_idempotent_impl(patient_id, menu_id, notes, actor_id, None).await
    }

    async fn create_order_idempotent(
        &self,
        patient_id: i64,
        menu_id: i64,
        notes: &str,
        actor_id: i64,
        idempotency_key: Option<&str>,
    ) -> Result<i64, ApiError> {
        self.create_order_idempotent_impl(patient_id, menu_id, notes, actor_id, idempotency_key).await
    }

    async fn get_order(&self, order_id: i64) -> Result<Option<OrderRecord>, ApiError> {
        self.get_order_impl(order_id).await
    }

    async fn set_order_status_if_version(
        &self,
        order_id: i64,
        expected_version: i32,
        next_status: &str,
        reason: Option<&str>,
    ) -> Result<bool, ApiError> {
        self.set_order_status_if_version_impl(order_id, expected_version, next_status, reason).await
    }

    async fn list_orders(&self, user_id: i64, role_name: &str, limit: i64, offset: i64) -> Result<Vec<OrderDto>, ApiError> {
        self.list_orders_impl(user_id, role_name, limit, offset).await
    }

    async fn add_order_note(&self, order_id: i64, note: &str, staff_user_id: i64) -> Result<(), ApiError> {
        self.add_order_note_impl(order_id, note, staff_user_id).await
    }

    async fn list_order_notes(&self, order_id: i64) -> Result<Vec<OrderNoteDto>, ApiError> {
        self.list_order_notes_impl(order_id).await
    }

    async fn add_ticket_split(&self, order_id: i64, split_by: &str, split_value: &str, quantity: i32) -> Result<(), ApiError> {
        self.add_ticket_split_impl(order_id, split_by, split_value, quantity).await
    }

    async fn list_ticket_splits(&self, order_id: i64) -> Result<Vec<TicketSplitDto>, ApiError> {
        self.list_ticket_splits_impl(order_id).await
    }

    async fn list_dish_categories(&self) -> Result<Vec<DishCategoryDto>, ApiError> {
        self.list_dish_categories_impl().await
    }

    async fn create_dish(
        &self,
        category_id: i64,
        name: &str,
        description: &str,
        base_price_cents: i32,
        photo_path: &str,
        actor_id: i64,
    ) -> Result<i64, ApiError> {
        self.create_dish_impl(category_id, name, description, base_price_cents, photo_path, actor_id).await
    }

    async fn list_dishes(&self) -> Result<Vec<DishDto>, ApiError> {
        self.list_dishes_impl().await
    }

    async fn set_dish_status(&self, dish_id: i64, is_published: bool, is_sold_out: bool) -> Result<(), ApiError> {
        self.set_dish_status_impl(dish_id, is_published, is_sold_out).await
    }

    async fn add_dish_option(&self, dish_id: i64, option_group: &str, option_value: &str, delta_price_cents: i32) -> Result<(), ApiError> {
        self.add_dish_option_impl(dish_id, option_group, option_value, delta_price_cents).await
    }

    async fn add_sales_window(&self, dish_id: i64, slot_name: &str, start_hhmm: &str, end_hhmm: &str) -> Result<(), ApiError> {
        self.add_sales_window_impl(dish_id, slot_name, start_hhmm, end_hhmm).await
    }

    async fn upsert_ranking_rule(&self, rule_key: &str, weight: f64, enabled: bool, actor_id: i64) -> Result<(), ApiError> {
        self.upsert_ranking_rule_impl(rule_key, weight, enabled, actor_id).await
    }

    async fn list_ranking_rules(&self) -> Result<Vec<RankingRuleDto>, ApiError> {
        self.list_ranking_rules_impl().await
    }

    async fn recommendations(&self) -> Result<Vec<RecommendationDto>, ApiError> {
        self.recommendations_impl().await
    }

    async fn create_campaign(
        &self,
        title: &str,
        dish_id: i64,
        success_threshold: i32,
        success_deadline_at: &str,
        actor_id: i64,
    ) -> Result<i64, ApiError> {
        self.create_campaign_impl(title, dish_id, success_threshold, success_deadline_at, actor_id).await
    }

    async fn join_campaign(&self, campaign_id: i64, user_id: i64) -> Result<(), ApiError> {
        self.join_campaign_impl(campaign_id, user_id).await
    }

    async fn list_campaigns(&self) -> Result<Vec<CampaignDto>, ApiError> {
        self.list_campaigns_impl().await
    }

    async fn close_inactive_campaigns(&self) -> Result<(), ApiError> {
        self.close_inactive_campaigns_impl().await
    }

    async fn create_governance_record(
        &self,
        tier: &str,
        lineage_source_id: Option<i64>,
        lineage_metadata: &str,
        payload_json: &str,
        actor_id: i64,
    ) -> Result<i64, ApiError> {
        self.create_governance_record_impl(tier, lineage_source_id, lineage_metadata, payload_json, actor_id).await
    }

    async fn list_governance_records(&self) -> Result<Vec<GovernanceRecordDto>, ApiError> {
        self.list_governance_records_impl().await
    }

    async fn tombstone_governance_record(&self, record_id: i64, reason: &str) -> Result<(), ApiError> {
        self.tombstone_governance_record_impl(record_id, reason).await
    }

    async fn create_telemetry_event(&self, experiment_key: &str, user_id: i64, event_name: &str, payload_json: &str) -> Result<(), ApiError> {
        self.create_telemetry_event_impl(experiment_key, user_id, event_name, payload_json).await
    }

    async fn append_audit(
        &self,
        action_type: &str,
        entity_type: &str,
        entity_id: &str,
        details_json: &str,
        actor_id: i64,
    ) -> Result<(), ApiError> {
        self.append_audit_impl(action_type, entity_type, entity_id, details_json, actor_id).await
    }

    async fn list_audits(&self) -> Result<Vec<AuditLogDto>, ApiError> {
        self.list_audits_impl().await
    }

    async fn list_retention_policies(&self) -> Result<Vec<RetentionPolicyDto>, ApiError> {
        self.list_retention_policies_impl().await
    }

    async fn upsert_retention_policy(&self, policy_key: &str, years: i32, actor_id: i64) -> Result<(), ApiError> {
        self.upsert_retention_policy_impl(policy_key, years, actor_id).await
    }

    async fn create_experiment(&self, experiment_key: &str) -> Result<i64, ApiError> {
        self.create_experiment_impl(experiment_key).await
    }

    async fn add_experiment_variant(
        &self,
        experiment_id: i64,
        variant_key: &str,
        allocation_weight: f64,
        feature_version: &str,
    ) -> Result<(), ApiError> {
        self.add_experiment_variant_impl(experiment_id, variant_key, allocation_weight, feature_version).await
    }

    async fn assign_experiment_variant(
        &self,
        experiment_id: i64,
        user_id: i64,
        mode: &str,
    ) -> Result<Option<String>, ApiError> {
        self.assign_experiment_impl(experiment_id, user_id, mode).await
    }

    async fn record_experiment_backtrack(
        &self,
        experiment_id: i64,
        from_version: &str,
        to_version: &str,
        reason: &str,
        actor_id: i64,
    ) -> Result<(), ApiError> {
        self.backtrack_experiment_impl(experiment_id, from_version, to_version, reason, actor_id).await
    }

    async fn funnel_metrics(&self) -> Result<Vec<FunnelMetricsDto>, ApiError> {
        self.funnel_metrics_impl().await
    }

    async fn retention_metrics(&self) -> Result<Vec<RetentionMetricsDto>, ApiError> {
        self.retention_metrics_impl().await
    }

    async fn recommendation_kpi(&self) -> Result<RecommendationKpiDto, ApiError> {
        self.recommendation_kpi_impl().await
    }

    async fn create_ingestion_task(
        &self,
        actor_id: i64,
        req: IngestionTaskCreateRequest,
    ) -> Result<i64, ApiError> {
        self.create_ingestion_task_impl(actor_id, req).await
    }

    async fn update_ingestion_task(
        &self,
        task_id: i64,
        actor_id: i64,
        actor_role: &str,
        req: IngestionTaskUpdateRequest,
    ) -> Result<i32, ApiError> {
        self.update_ingestion_task_impl(task_id, actor_id, actor_role, req).await
    }

    async fn rollback_ingestion_task(
        &self,
        task_id: i64,
        actor_id: i64,
        actor_role: &str,
        req: IngestionTaskRollbackRequest,
    ) -> Result<i32, ApiError> {
        self.rollback_ingestion_task_impl(task_id, actor_id, actor_role, req).await
    }

    async fn run_ingestion_task(&self, task_id: i64, actor_id: i64, actor_role: &str) -> Result<(), ApiError> {
        self.run_ingestion_task_impl(task_id, actor_id, actor_role).await
    }

    async fn list_ingestion_tasks(&self, actor_id: i64, actor_role: &str) -> Result<Vec<IngestionTaskDto>, ApiError> {
        self.list_ingestion_tasks_impl(actor_id, actor_role).await
    }

    async fn ingestion_task_versions(
        &self,
        task_id: i64,
        actor_id: i64,
        actor_role: &str,
    ) -> Result<Vec<IngestionTaskVersionDto>, ApiError> {
        self.ingestion_task_versions_impl(task_id, actor_id, actor_role).await
    }

    async fn ingestion_task_runs(
        &self,
        task_id: i64,
        limit: i64,
        actor_id: i64,
        actor_role: &str,
    ) -> Result<Vec<IngestionTaskRunDto>, ApiError> {
        self.ingestion_task_runs_impl(task_id, limit, actor_id, actor_role).await
    }
}
