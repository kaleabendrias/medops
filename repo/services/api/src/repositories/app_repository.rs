use async_trait::async_trait;
use serde::Serialize;
use contracts::{
    AttachmentMetadataDto, AuditLogDto, BedDto, BedEventDto, CampaignDto, DiningMenuDto,
    DishCategoryDto, DishDto, FunnelMetricsDto, GovernanceRecordDto, HospitalDto,
    IngestionTaskCreateRequest, IngestionTaskDto, IngestionTaskRollbackRequest,
    IngestionTaskRunDto, IngestionTaskUpdateRequest, IngestionTaskVersionDto,
    MenuEntitlementDto, OrderDto, OrderNoteDto, PatientProfileDto, PatientSearchResultDto,
    RecommendationKpiDto, RecommendationDto, RetentionMetricsDto, RetentionPolicyDto,
    RevisionTimelineDto, RoleDto, TicketSplitDto, UserSummaryDto,
};

use crate::contracts::ApiError;

#[derive(Debug, Clone)]
pub struct UserAuthRecord {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
    pub role_name: String,
    pub disabled: bool,
    pub failed_attempts: i32,
    pub locked_now: bool,
}

#[derive(Debug, Clone)]
pub struct SessionRecord {
    pub user_id: i64,
    pub username: String,
    pub role_name: String,
    pub disabled: bool,
    pub inactive_expired: bool,
}

#[derive(Debug, Clone)]
pub struct OrderRecord {
    pub id: i64,
    pub patient_id: i64,
    pub menu_id: i64,
    pub status: String,
    pub notes: String,
    pub version: i32,
    pub created_by: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PatientSensitiveRecord {
    pub id: i64,
    pub mrn: String,
    pub first_name: String,
    pub last_name: String,
    pub birth_date: String,
    pub gender: String,
    pub phone: String,
    pub email: String,
    pub allergies: String,
    pub contraindications: String,
    pub history: String,
}

/// Atomic bed transition request shipped from the service layer to the
/// repository. The repository is responsible for executing every check
/// (state-machine, patient existence, occupancy invariants, target-bed
/// eligibility for transfer/swap) and every mutation (bed state updates,
/// occupancy rows, audit-shaped bed_event row) inside a SINGLE database
/// transaction. If any prerequisite fails, the transaction rolls back and
/// the underlying state is left untouched.
#[derive(Debug, Clone)]
pub struct BedTransitionDbRequest {
    pub bed_id: i64,
    pub action: String,
    pub target_state: String,
    pub related_bed_id: Option<i64>,
    pub patient_id: Option<i64>,
    pub note: String,
    pub actor_id: i64,
}

#[derive(Debug, Clone)]
pub struct AttachmentStorageRecord {
    pub mime_type: String,
    pub payload_bytes: Vec<u8>,
}

#[async_trait]
pub trait AppRepository: Send + Sync {
    async fn list_hospitals(&self) -> Result<Vec<HospitalDto>, ApiError>;
    async fn list_roles(&self) -> Result<Vec<RoleDto>, ApiError>;
    async fn get_user_auth(&self, username: &str) -> Result<Option<UserAuthRecord>, ApiError>;
    async fn update_user_password_hash(&self, user_id: i64, new_hash: &str) -> Result<(), ApiError>;
    async fn register_failed_login(&self, user_id: i64, attempt_count: i32) -> Result<(), ApiError>;
    async fn reset_login_failures(&self, user_id: i64) -> Result<(), ApiError>;
    async fn create_session(&self, token: &str, user_id: i64) -> Result<(), ApiError>;
    async fn get_session(&self, token: &str) -> Result<Option<SessionRecord>, ApiError>;
    async fn touch_session(&self, token: &str) -> Result<(), ApiError>;
    async fn delete_session(&self, token: &str) -> Result<(), ApiError>;
    async fn revoke_user_sessions(&self, user_id: i64) -> Result<(), ApiError>;
    async fn user_has_permission(&self, role_name: &str, permission_key: &str) -> Result<bool, ApiError>;
    async fn list_menu_entitlements(&self, role_name: &str) -> Result<Vec<MenuEntitlementDto>, ApiError>;
    async fn disable_user(&self, user_id: i64) -> Result<(), ApiError>;
    async fn list_users(&self) -> Result<Vec<UserSummaryDto>, ApiError>;

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
    ) -> Result<i64, ApiError>;
    async fn can_access_patient(&self, user_id: i64, role_name: &str, patient_id: i64) -> Result<bool, ApiError>;
    async fn assign_patient(&self, patient_id: i64, target_user_id: i64, assignment_type: &str, assigned_by: i64) -> Result<(), ApiError>;
    async fn get_patient(&self, patient_id: i64) -> Result<Option<PatientProfileDto>, ApiError>;
    async fn get_patient_sensitive(&self, patient_id: i64) -> Result<Option<PatientSensitiveRecord>, ApiError>;
    async fn list_patients(&self, user_id: i64, role_name: &str, limit: i64, offset: i64) -> Result<Vec<PatientProfileDto>, ApiError>;
    async fn update_patient_demographics(
        &self,
        patient_id: i64,
        first_name: &str,
        last_name: &str,
        birth_date: &str,
        gender: &str,
        phone: &str,
        email: &str,
    ) -> Result<(), ApiError>;
    async fn update_patient_clinical_field(&self, patient_id: i64, field_name: &str, value: &str) -> Result<(), ApiError>;
    async fn add_patient_visit_note(&self, patient_id: i64, note: &str, actor_id: i64) -> Result<(), ApiError>;
    async fn list_patient_revisions(&self, patient_id: i64) -> Result<Vec<RevisionTimelineDto>, ApiError>;
    async fn create_patient_revision(
        &self,
        patient_id: i64,
        entity_type: &str,
        before_json: &str,
        after_json: &str,
        reason: &str,
        actor_id: i64,
    ) -> Result<(), ApiError>;
    async fn create_attachment_metadata(
        &self,
        patient_id: i64,
        file_name: &str,
        mime_type: &str,
        file_size_bytes: i64,
        payload_bytes: &[u8],
        uploaded_by: i64,
    ) -> Result<(), ApiError>;
    async fn list_attachments(&self, patient_id: i64) -> Result<Vec<AttachmentMetadataDto>, ApiError>;
    async fn get_attachment_storage(
        &self,
        patient_id: i64,
        attachment_id: i64,
    ) -> Result<Option<AttachmentStorageRecord>, ApiError>;

    async fn list_beds(&self) -> Result<Vec<BedDto>, ApiError>;
    async fn get_bed_state(&self, bed_id: i64) -> Result<Option<String>, ApiError>;
    async fn set_bed_state(&self, bed_id: i64, state: &str) -> Result<(), ApiError>;
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
    ) -> Result<(), ApiError>;
    async fn list_bed_events(&self) -> Result<Vec<BedEventDto>, ApiError>;
    async fn check_in_patient(&self, bed_id: i64, patient_id: i64) -> Result<(), ApiError>;
    async fn check_out_patient(&self, bed_id: i64, reason: &str) -> Result<(), ApiError>;
    async fn active_bed_occupant(&self, bed_id: i64) -> Result<Option<i64>, ApiError>;
    /// Atomically apply a bed transition. The implementation MUST run all
    /// validation and mutation inside a single database transaction so a
    /// failed prerequisite leaves the bed state and occupancy records
    /// untouched.
    async fn apply_bed_transition(&self, req: BedTransitionDbRequest) -> Result<(), ApiError>;

    async fn create_menu(&self, menu_date: &str, meal_period: &str, item_name: &str, calories: i32, actor_id: i64) -> Result<(), ApiError>;
    async fn list_menus(&self) -> Result<Vec<DiningMenuDto>, ApiError>;
    /// Pre-flight menu governance enforcement for the order creation flow.
    /// Returns Err(ApiError::BadRequest) if the menu line / linked dish does
    /// not exist, and Err(ApiError::Forbidden) if the dish is unpublished,
    /// sold out, or outside its configured sales window.
    async fn validate_menu_orderable(&self, menu_id: i64) -> Result<(), ApiError>;
    async fn create_order(&self, patient_id: i64, menu_id: i64, notes: &str, actor_id: i64) -> Result<i64, ApiError>;
    async fn create_order_idempotent(
        &self,
        patient_id: i64,
        menu_id: i64,
        notes: &str,
        actor_id: i64,
        idempotency_key: Option<&str>,
    ) -> Result<i64, ApiError>;
    async fn get_order(&self, order_id: i64) -> Result<Option<OrderRecord>, ApiError>;
    async fn set_order_status_if_version(
        &self,
        order_id: i64,
        expected_version: i32,
        next_status: &str,
        reason: Option<&str>,
    ) -> Result<bool, ApiError>;
    async fn list_orders(&self, user_id: i64, role_name: &str, limit: i64, offset: i64) -> Result<Vec<OrderDto>, ApiError>;

    async fn create_governance_record(
        &self,
        tier: &str,
        lineage_source_id: Option<i64>,
        lineage_metadata: &str,
        payload_json: &str,
        actor_id: i64,
    ) -> Result<i64, ApiError>;
    async fn list_governance_records(&self) -> Result<Vec<GovernanceRecordDto>, ApiError>;
    async fn tombstone_governance_record(&self, record_id: i64, reason: &str) -> Result<(), ApiError>;

    async fn create_telemetry_event(&self, experiment_key: &str, user_id: i64, event_name: &str, payload_json: &str) -> Result<(), ApiError>;

    async fn append_audit(
        &self,
        action_type: &str,
        entity_type: &str,
        entity_id: &str,
        details_json: &str,
        actor_id: i64,
    ) -> Result<(), ApiError>;
    async fn list_audits(&self) -> Result<Vec<AuditLogDto>, ApiError>;

    async fn list_retention_policies(&self) -> Result<Vec<RetentionPolicyDto>, ApiError>;
    async fn upsert_retention_policy(&self, policy_key: &str, years: i32, actor_id: i64) -> Result<(), ApiError>;

    async fn search_patients(
        &self,
        user_id: i64,
        role_name: &str,
        query: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<PatientSearchResultDto>, ApiError>;

    async fn list_dish_categories(&self) -> Result<Vec<DishCategoryDto>, ApiError>;
    async fn create_dish(
        &self,
        category_id: i64,
        name: &str,
        description: &str,
        base_price_cents: i32,
        photo_path: &str,
        actor_id: i64,
    ) -> Result<i64, ApiError>;
    async fn set_dish_status(&self, dish_id: i64, is_published: bool, is_sold_out: bool) -> Result<(), ApiError>;
    async fn add_dish_option(&self, dish_id: i64, option_group: &str, option_value: &str, delta_price_cents: i32) -> Result<(), ApiError>;
    async fn add_sales_window(&self, dish_id: i64, slot_name: &str, start_hhmm: &str, end_hhmm: &str) -> Result<(), ApiError>;
    async fn list_dishes(&self) -> Result<Vec<DishDto>, ApiError>;
    async fn upsert_ranking_rule(&self, rule_key: &str, weight: f64, enabled: bool, actor_id: i64) -> Result<(), ApiError>;
    async fn list_ranking_rules(&self) -> Result<Vec<contracts::RankingRuleDto>, ApiError>;
    async fn recommendations(&self) -> Result<Vec<RecommendationDto>, ApiError>;

    async fn close_inactive_campaigns(&self) -> Result<(), ApiError>;
    async fn create_campaign(
        &self,
        title: &str,
        dish_id: i64,
        success_threshold: i32,
        success_deadline_at: &str,
        actor_id: i64,
    ) -> Result<i64, ApiError>;
    async fn join_campaign(&self, campaign_id: i64, user_id: i64) -> Result<(), ApiError>;
    async fn list_campaigns(&self) -> Result<Vec<CampaignDto>, ApiError>;

    async fn add_ticket_split(&self, order_id: i64, split_by: &str, split_value: &str, quantity: i32) -> Result<(), ApiError>;
    async fn list_ticket_splits(&self, order_id: i64) -> Result<Vec<TicketSplitDto>, ApiError>;
    async fn add_order_note(&self, order_id: i64, note: &str, staff_user_id: i64) -> Result<(), ApiError>;
    async fn list_order_notes(&self, order_id: i64) -> Result<Vec<OrderNoteDto>, ApiError>;

    async fn create_experiment(&self, experiment_key: &str) -> Result<i64, ApiError>;
    async fn add_experiment_variant(
        &self,
        experiment_id: i64,
        variant_key: &str,
        allocation_weight: f64,
        feature_version: &str,
    ) -> Result<(), ApiError>;
    async fn assign_experiment_variant(
        &self,
        experiment_id: i64,
        user_id: i64,
        mode: &str,
    ) -> Result<Option<String>, ApiError>;
    async fn record_experiment_backtrack(
        &self,
        experiment_id: i64,
        from_version: &str,
        to_version: &str,
        reason: &str,
        actor_id: i64,
    ) -> Result<(), ApiError>;

    async fn funnel_metrics(&self) -> Result<Vec<FunnelMetricsDto>, ApiError>;
    async fn retention_metrics(&self) -> Result<Vec<RetentionMetricsDto>, ApiError>;
    async fn recommendation_kpi(&self) -> Result<RecommendationKpiDto, ApiError>;

    async fn create_ingestion_task(
        &self,
        actor_id: i64,
        req: IngestionTaskCreateRequest,
    ) -> Result<i64, ApiError>;
    async fn update_ingestion_task(
        &self,
        task_id: i64,
        actor_id: i64,
        actor_role: &str,
        req: IngestionTaskUpdateRequest,
    ) -> Result<i32, ApiError>;
    async fn rollback_ingestion_task(
        &self,
        task_id: i64,
        actor_id: i64,
        actor_role: &str,
        req: IngestionTaskRollbackRequest,
    ) -> Result<i32, ApiError>;
    async fn run_ingestion_task(&self, task_id: i64, actor_id: i64, actor_role: &str) -> Result<(), ApiError>;
    async fn list_ingestion_tasks(&self, actor_id: i64, actor_role: &str) -> Result<Vec<IngestionTaskDto>, ApiError>;
    async fn ingestion_task_versions(
        &self,
        task_id: i64,
        actor_id: i64,
        actor_role: &str,
    ) -> Result<Vec<IngestionTaskVersionDto>, ApiError>;
    async fn ingestion_task_runs(
        &self,
        task_id: i64,
        limit: i64,
        actor_id: i64,
        actor_role: &str,
    ) -> Result<Vec<IngestionTaskRunDto>, ApiError>;
}
