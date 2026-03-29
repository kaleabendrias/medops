use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub db: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleDto {
    pub id: i64,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HospitalDto {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub city: String,
    pub country: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionSettingsDto {
    pub audit_log_days: u32,
    pub session_days: u32,
    pub patient_record_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSettingsDto {
    pub cookie_name: String,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: String,
    pub ttl_minutes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthLoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthLoginResponse {
    pub token: String,
    pub user_id: i64,
    pub username: String,
    pub role: String,
    pub expires_in_minutes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MenuEntitlementDto {
    pub menu_key: String,
    pub allowed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSummaryDto {
    pub id: i64,
    pub username: String,
    pub role: String,
    pub disabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatientCreateRequest {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatientUpdateRequest {
    pub first_name: String,
    pub last_name: String,
    pub birth_date: String,
    pub gender: String,
    pub phone: String,
    pub email: String,
    pub reason_for_change: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClinicalEditRequest {
    pub value: String,
    pub reason_for_change: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisitNoteRequest {
    pub note: String,
    pub reason_for_change: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatientAssignRequest {
    pub target_user_id: i64,
    pub assignment_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatientProfileDto {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevisionTimelineDto {
    pub id: i64,
    pub entity_type: String,
    pub diff_before: String,
    pub diff_after: String,
    #[serde(default)]
    pub field_deltas_json: String,
    pub reason_for_change: String,
    pub actor_username: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentMetadataDto {
    pub id: i64,
    pub file_name: String,
    pub mime_type: String,
    pub file_size_bytes: i64,
    pub uploaded_by: String,
    pub uploaded_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatientExportDto {
    pub format: String,
    pub content: String,
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedDto {
    pub id: i64,
    pub building: String,
    pub unit: String,
    pub room: String,
    pub bed_label: String,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedTransitionRequest {
    pub action: String,
    pub target_state: String,
    pub related_bed_id: Option<i64>,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedEventDto {
    pub id: i64,
    pub action: String,
    pub from_bed_id: Option<i64>,
    pub to_bed_id: Option<i64>,
    pub from_state: Option<String>,
    pub to_state: Option<String>,
    pub actor_username: String,
    pub occurred_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiningMenuRequest {
    pub menu_date: String,
    pub meal_period: String,
    pub item_name: String,
    pub calories: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiningMenuDto {
    pub id: i64,
    pub menu_date: String,
    pub meal_period: String,
    pub item_name: String,
    pub calories: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderCreateRequest {
    pub patient_id: i64,
    pub menu_id: i64,
    pub notes: String,
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderStatusRequest {
    pub status: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub expected_version: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderDto {
    pub id: i64,
    pub patient_id: i64,
    pub menu_id: i64,
    pub status: String,
    pub notes: String,
    pub version: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestionTaskCreateRequest {
    pub task_name: String,
    pub seed_urls: Vec<String>,
    pub extraction_rules_json: String,
    pub pagination_strategy: String,
    pub max_depth: i32,
    pub incremental_field: Option<String>,
    pub schedule_cron: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestionTaskUpdateRequest {
    pub seed_urls: Vec<String>,
    pub extraction_rules_json: String,
    pub pagination_strategy: String,
    pub max_depth: i32,
    pub incremental_field: Option<String>,
    pub schedule_cron: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestionTaskRollbackRequest {
    pub target_version: i32,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestionTaskDto {
    pub id: i64,
    pub task_name: String,
    pub status: String,
    pub active_version: i32,
    pub schedule_cron: String,
    pub max_depth: i32,
    pub pagination_strategy: String,
    pub incremental_field: Option<String>,
    pub next_run_at: Option<String>,
    pub last_run_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestionTaskVersionDto {
    pub task_id: i64,
    pub version_number: i32,
    pub seed_urls_json: String,
    pub extraction_rules_json: String,
    pub rollback_of_version: Option<i32>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestionTaskRunDto {
    pub id: i64,
    pub task_id: i64,
    pub task_version: i32,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub records_extracted: i32,
    pub diagnostics_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceRecordRequest {
    pub tier: String,
    pub lineage_source_id: Option<i64>,
    pub lineage_metadata: String,
    pub payload_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceDeleteRequest {
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceRecordDto {
    pub id: i64,
    pub tier: String,
    pub lineage_source_id: Option<i64>,
    pub lineage_metadata: String,
    pub payload_json: String,
    pub tombstoned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryEventRequest {
    pub experiment_key: String,
    pub event_name: String,
    pub payload_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogDto {
    pub id: i64,
    pub action_type: String,
    pub entity_type: String,
    pub entity_id: String,
    pub actor_username: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicyDto {
    pub policy_key: String,
    pub years: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatientSearchResultDto {
    pub id: i64,
    pub mrn: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DishCategoryDto {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DishCreateRequest {
    pub category_id: i64,
    pub name: String,
    pub description: String,
    pub base_price_cents: i32,
    pub photo_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DishStatusRequest {
    pub is_published: bool,
    pub is_sold_out: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DishOptionRequest {
    pub option_group: String,
    pub option_value: String,
    pub delta_price_cents: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DishWindowRequest {
    pub slot_name: String,
    pub start_hhmm: String,
    pub end_hhmm: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DishDto {
    pub id: i64,
    pub category: String,
    pub name: String,
    pub description: String,
    pub base_price_cents: i32,
    pub photo_path: String,
    pub is_published: bool,
    pub is_sold_out: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankingRuleRequest {
    pub rule_key: String,
    pub weight: f64,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankingRuleDto {
    pub rule_key: String,
    pub weight: f64,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendationDto {
    pub dish_id: i64,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignCreateRequest {
    pub title: String,
    pub dish_id: i64,
    pub success_threshold: i32,
    pub success_deadline_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignDto {
    pub id: i64,
    pub title: String,
    pub dish_id: i64,
    pub success_threshold: i32,
    pub success_deadline_at: String,
    pub status: String,
    pub participants: i32,
    pub qualifying_orders: i32,
    pub last_activity_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketSplitRequest {
    pub split_by: String,
    pub split_value: String,
    pub quantity: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketSplitDto {
    pub id: i64,
    pub split_by: String,
    pub split_value: String,
    pub quantity: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderNoteRequest {
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderNoteDto {
    pub id: i64,
    pub note: String,
    pub staff_username: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentCreateRequest {
    pub experiment_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentVariantRequest {
    pub variant_key: String,
    pub allocation_weight: f64,
    pub feature_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentAssignRequest {
    pub user_id: i64,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentBacktrackRequest {
    pub from_version: String,
    pub to_version: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunnelMetricsDto {
    pub step: String,
    pub users: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionMetricsDto {
    pub cohort: String,
    pub retained_users: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendationKpiDto {
    pub ctr: f64,
    pub conversion: f64,
}
