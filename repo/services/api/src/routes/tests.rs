//! Controller-layer unit tests. Each test spins up an in-process Rocket
//! client backed by a StubRepo so no real database or network is required.
#![allow(clippy::too_many_arguments)]
use std::sync::Arc;

use async_trait::async_trait;
use rocket::http::{Cookie, Header, Status};
use rocket::local::asynchronous::Client;

use crate::config::{AuthPolicyConfig, RetentionConfig};
use crate::contracts::{ApiError, RetentionSnapshot, SessionSnapshot};
use crate::repositories::app_repository::{
    AppRepository, AttachmentStorageRecord, BedTransitionDbRequest, OrderRecord,
    PatientSensitiveRecord, SessionRecord, UserAuthRecord,
};
use crate::services::app_service::AppService;
use crate::AppState;
use contracts::{
    AttachmentMetadataDto, AuditLogDto, BedDto, BedEventDto, CampaignDto, DiningMenuDto,
    DishCategoryDto, DishDto, FunnelMetricsDto, GovernanceRecordDto, HospitalDto,
    IngestionTaskCreateRequest, IngestionTaskDto, IngestionTaskRollbackRequest,
    IngestionTaskRunDto, IngestionTaskUpdateRequest, IngestionTaskVersionDto,
    MenuEntitlementDto, OrderDto, OrderNoteDto, PatientProfileDto, PatientSearchResultDto,
    RecommendationDto, RecommendationKpiDto, RetentionMetricsDto, RetentionPolicyDto,
    RevisionTimelineDto, RoleDto, TicketSplitDto, UserSummaryDto,
};

// ── Stub repository ───────────────────────────────────────────────────────────

/// In-memory stub. All methods return `Err(ApiError::Internal)` by default;
/// individual tests configure only the fields they need.
struct StubRepo {
    user: Option<UserAuthRecord>,
    session: Option<SessionRecord>,
    permission: bool,
    /// When true, list/read methods for governance, ingestion, and cafeteria
    /// return `Ok(vec![])` instead of `Err(Internal)`.
    reads_ok: bool,
}

impl StubRepo {
    fn new() -> Self {
        Self { user: None, session: None, permission: false, reads_ok: false }
    }
    fn with_user(mut self, u: UserAuthRecord) -> Self { self.user = Some(u); self }
    fn with_session(mut self, s: SessionRecord) -> Self { self.session = Some(s); self }
    fn with_permission(mut self) -> Self { self.permission = true; self }
    fn with_reads_ok(mut self) -> Self { self.reads_ok = true; self }
}

#[async_trait]
impl AppRepository for StubRepo {
    // Auth
    async fn get_user_auth(&self, _: &str) -> Result<Option<UserAuthRecord>, ApiError> { Ok(self.user.clone()) }
    async fn update_user_password_hash(&self, _: i64, _: &str) -> Result<(), ApiError> { Ok(()) }
    async fn register_failed_login(&self, _: i64, _: i32) -> Result<(), ApiError> { Ok(()) }
    async fn reset_login_failures(&self, _: i64) -> Result<(), ApiError> { Ok(()) }
    async fn create_session(&self, _: &str, _: i64) -> Result<(), ApiError> { Ok(()) }
    async fn get_session(&self, _: &str) -> Result<Option<SessionRecord>, ApiError> { Ok(self.session.clone()) }
    async fn touch_session(&self, _: &str) -> Result<(), ApiError> { Ok(()) }
    async fn delete_session(&self, _: &str) -> Result<(), ApiError> { Ok(()) }
    async fn revoke_user_sessions(&self, _: i64) -> Result<(), ApiError> { Ok(()) }
    async fn user_has_permission(&self, _: &str, _: &str) -> Result<bool, ApiError> { Ok(self.permission) }
    async fn list_menu_entitlements(&self, _: &str) -> Result<Vec<MenuEntitlementDto>, ApiError> { Ok(vec![]) }
    async fn disable_user(&self, _: i64) -> Result<(), ApiError> { Err(ApiError::Internal) }
    async fn list_users(&self) -> Result<Vec<UserSummaryDto>, ApiError> { Err(ApiError::Internal) }
    async fn append_audit(&self, _: &str, _: &str, _: &str, _: &str, _: i64) -> Result<(), ApiError> { Ok(()) }

    // Catalog
    async fn list_hospitals(&self) -> Result<Vec<HospitalDto>, ApiError> { Err(ApiError::Internal) }
    async fn list_roles(&self) -> Result<Vec<RoleDto>, ApiError> { Err(ApiError::Internal) }

    // Patients
    async fn create_patient(&self, _: i64, _: &str, _: &str, _: &str, _: &str, _: &str, _: &str, _: &str, _: &str, _: &str, _: &str) -> Result<i64, ApiError> { Err(ApiError::Internal) }
    async fn can_access_patient(&self, _: i64, _: &str, _: i64) -> Result<bool, ApiError> { Err(ApiError::Internal) }
    async fn assign_patient(&self, _: i64, _: i64, _: &str, _: i64) -> Result<(), ApiError> { Err(ApiError::Internal) }
    async fn get_patient(&self, _: i64) -> Result<Option<PatientProfileDto>, ApiError> { Err(ApiError::Internal) }
    async fn get_patient_sensitive(&self, _: i64) -> Result<Option<PatientSensitiveRecord>, ApiError> { Err(ApiError::Internal) }
    async fn list_patients(&self, _: i64, _: &str, _: i64, _: i64) -> Result<Vec<PatientProfileDto>, ApiError> { Err(ApiError::Internal) }
    async fn update_patient_demographics(&self, _: i64, _: &str, _: &str, _: &str, _: &str, _: &str, _: &str) -> Result<(), ApiError> { Err(ApiError::Internal) }
    async fn update_patient_clinical_field(&self, _: i64, _: &str, _: &str) -> Result<(), ApiError> { Err(ApiError::Internal) }
    async fn add_patient_visit_note(&self, _: i64, _: &str, _: i64) -> Result<(), ApiError> { Err(ApiError::Internal) }
    async fn list_patient_revisions(&self, _: i64) -> Result<Vec<RevisionTimelineDto>, ApiError> { Err(ApiError::Internal) }
    async fn create_patient_revision(&self, _: i64, _: &str, _: &str, _: &str, _: &str, _: i64) -> Result<(), ApiError> { Err(ApiError::Internal) }
    async fn create_attachment_metadata(&self, _: i64, _: &str, _: &str, _: i64, _: &[u8], _: i64) -> Result<(), ApiError> { Err(ApiError::Internal) }
    async fn list_attachments(&self, _: i64) -> Result<Vec<AttachmentMetadataDto>, ApiError> { Err(ApiError::Internal) }
    async fn get_attachment_storage(&self, _: i64, _: i64) -> Result<Option<AttachmentStorageRecord>, ApiError> { Err(ApiError::Internal) }
    async fn search_patients(&self, _: i64, _: &str, _: &str, _: i64, _: i64) -> Result<Vec<PatientSearchResultDto>, ApiError> { Err(ApiError::Internal) }

    // Bedboard
    async fn list_beds(&self) -> Result<Vec<BedDto>, ApiError> { Err(ApiError::Internal) }
    async fn get_bed_state(&self, _: i64) -> Result<Option<String>, ApiError> { Err(ApiError::Internal) }
    async fn set_bed_state(&self, _: i64, _: &str) -> Result<(), ApiError> { Err(ApiError::Internal) }
    async fn record_bed_event(&self, _: &str, _: Option<i64>, _: Option<i64>, _: Option<&str>, _: Option<&str>, _: i64, _: &str, _: Option<i64>) -> Result<(), ApiError> { Err(ApiError::Internal) }
    async fn list_bed_events(&self) -> Result<Vec<BedEventDto>, ApiError> { Err(ApiError::Internal) }
    async fn check_in_patient(&self, _: i64, _: i64) -> Result<(), ApiError> { Err(ApiError::Internal) }
    async fn check_out_patient(&self, _: i64, _: &str) -> Result<(), ApiError> { Err(ApiError::Internal) }
    async fn active_bed_occupant(&self, _: i64) -> Result<Option<i64>, ApiError> { Err(ApiError::Internal) }
    async fn apply_bed_transition(&self, _: BedTransitionDbRequest) -> Result<(), ApiError> { Err(ApiError::Internal) }

    // Dining & Orders
    async fn create_menu(&self, _: &str, _: &str, _: &str, _: i32, _: i64) -> Result<(), ApiError> { Err(ApiError::Internal) }
    async fn list_menus(&self) -> Result<Vec<DiningMenuDto>, ApiError> { Err(ApiError::Internal) }
    async fn validate_menu_orderable(&self, _: i64) -> Result<(), ApiError> { Err(ApiError::Internal) }
    async fn create_order(&self, _: i64, _: i64, _: &str, _: i64) -> Result<i64, ApiError> { Err(ApiError::Internal) }
    async fn create_order_idempotent(&self, _: i64, _: i64, _: &str, _: i64, _: Option<&str>) -> Result<i64, ApiError> { Err(ApiError::Internal) }
    async fn get_order(&self, _: i64) -> Result<Option<OrderRecord>, ApiError> { Err(ApiError::Internal) }
    async fn set_order_status_if_version(&self, _: i64, _: i32, _: &str, _: Option<&str>) -> Result<bool, ApiError> { Err(ApiError::Internal) }
    async fn list_orders(&self, _: i64, _: &str, _: i64, _: i64) -> Result<Vec<OrderDto>, ApiError> { Err(ApiError::Internal) }

    // Governance / Telemetry / Audit
    async fn create_governance_record(&self, _: &str, _: Option<i64>, _: &str, _: &str, _: i64) -> Result<i64, ApiError> {
        if self.reads_ok { Ok(42) } else { Err(ApiError::Internal) }
    }
    async fn list_governance_records(&self) -> Result<Vec<GovernanceRecordDto>, ApiError> {
        if self.reads_ok { Ok(vec![]) } else { Err(ApiError::Internal) }
    }
    async fn tombstone_governance_record(&self, _: i64, _: &str) -> Result<(), ApiError> { Err(ApiError::Internal) }
    async fn create_telemetry_event(&self, _: &str, _: i64, _: &str, _: &str) -> Result<(), ApiError> { Ok(()) }
    async fn list_audits(&self) -> Result<Vec<AuditLogDto>, ApiError> { Err(ApiError::Internal) }

    // Retention
    async fn list_retention_policies(&self) -> Result<Vec<RetentionPolicyDto>, ApiError> { Ok(vec![]) }
    async fn upsert_retention_policy(&self, _: &str, _: i32, _: i64) -> Result<(), ApiError> { Ok(()) }

    // Catalog / Dishes
    async fn list_dish_categories(&self) -> Result<Vec<DishCategoryDto>, ApiError> {
        if self.reads_ok { Ok(vec![]) } else { Err(ApiError::Internal) }
    }
    async fn create_dish(&self, _: i64, _: &str, _: &str, _: i32, _: &str, _: i64) -> Result<i64, ApiError> { Err(ApiError::Internal) }
    async fn set_dish_status(&self, _: i64, _: bool, _: bool) -> Result<(), ApiError> { Err(ApiError::Internal) }
    async fn add_dish_option(&self, _: i64, _: &str, _: &str, _: i32) -> Result<(), ApiError> { Err(ApiError::Internal) }
    async fn add_sales_window(&self, _: i64, _: &str, _: &str, _: &str) -> Result<(), ApiError> { Err(ApiError::Internal) }
    async fn list_dishes(&self) -> Result<Vec<DishDto>, ApiError> {
        if self.reads_ok { Ok(vec![]) } else { Err(ApiError::Internal) }
    }
    async fn upsert_ranking_rule(&self, _: &str, _: f64, _: bool, _: i64) -> Result<(), ApiError> { Err(ApiError::Internal) }
    async fn list_ranking_rules(&self) -> Result<Vec<contracts::RankingRuleDto>, ApiError> {
        if self.reads_ok { Ok(vec![]) } else { Err(ApiError::Internal) }
    }
    async fn recommendations(&self) -> Result<Vec<RecommendationDto>, ApiError> { Err(ApiError::Internal) }

    // Campaigns
    async fn close_inactive_campaigns(&self) -> Result<(), ApiError> { Ok(()) }
    async fn create_campaign(&self, _: &str, _: i64, _: i32, _: &str, _: i64) -> Result<i64, ApiError> { Err(ApiError::Internal) }
    async fn join_campaign(&self, _: i64, _: i64) -> Result<(), ApiError> { Err(ApiError::Internal) }
    async fn list_campaigns(&self) -> Result<Vec<CampaignDto>, ApiError> { Err(ApiError::Internal) }

    // Ticket splits & order notes
    async fn add_ticket_split(&self, _: i64, _: &str, _: &str, _: i32) -> Result<(), ApiError> { Err(ApiError::Internal) }
    async fn list_ticket_splits(&self, _: i64) -> Result<Vec<TicketSplitDto>, ApiError> { Err(ApiError::Internal) }
    async fn add_order_note(&self, _: i64, _: &str, _: i64) -> Result<(), ApiError> { Err(ApiError::Internal) }
    async fn list_order_notes(&self, _: i64) -> Result<Vec<OrderNoteDto>, ApiError> { Err(ApiError::Internal) }

    // Experiments
    async fn create_experiment(&self, _: &str) -> Result<i64, ApiError> { Err(ApiError::Internal) }
    async fn add_experiment_variant(&self, _: i64, _: &str, _: f64, _: &str) -> Result<(), ApiError> { Err(ApiError::Internal) }
    async fn assign_experiment_variant(&self, _: i64, _: i64, _: &str) -> Result<Option<String>, ApiError> { Err(ApiError::Internal) }
    async fn record_experiment_backtrack(&self, _: i64, _: &str, _: &str, _: &str, _: i64) -> Result<(), ApiError> { Err(ApiError::Internal) }

    // Analytics
    async fn funnel_metrics(&self) -> Result<Vec<FunnelMetricsDto>, ApiError> { Err(ApiError::Internal) }
    async fn retention_metrics(&self) -> Result<Vec<RetentionMetricsDto>, ApiError> { Err(ApiError::Internal) }
    async fn recommendation_kpi(&self) -> Result<RecommendationKpiDto, ApiError> { Err(ApiError::Internal) }

    // Ingestion
    async fn create_ingestion_task(&self, _: i64, _: IngestionTaskCreateRequest) -> Result<i64, ApiError> { Err(ApiError::Internal) }
    async fn update_ingestion_task(&self, _: i64, _: i64, _: &str, _: IngestionTaskUpdateRequest) -> Result<i32, ApiError> { Err(ApiError::Internal) }
    async fn rollback_ingestion_task(&self, _: i64, _: i64, _: &str, _: IngestionTaskRollbackRequest) -> Result<i32, ApiError> { Err(ApiError::Internal) }
    async fn run_ingestion_task(&self, _: i64, _: i64, _: &str) -> Result<(), ApiError> { Err(ApiError::Internal) }
    async fn list_ingestion_tasks(&self, _: i64, _: &str) -> Result<Vec<IngestionTaskDto>, ApiError> {
        if self.reads_ok { Ok(vec![]) } else { Err(ApiError::Internal) }
    }
    async fn ingestion_task_versions(&self, _: i64, _: i64, _: &str) -> Result<Vec<IngestionTaskVersionDto>, ApiError> { Err(ApiError::Internal) }
    async fn ingestion_task_runs(&self, _: i64, _: i64, _: i64, _: &str) -> Result<Vec<IngestionTaskRunDto>, ApiError> { Err(ApiError::Internal) }
}

// ── Test helpers ─────────────────────────────────────────────────────────────

fn test_auth_policy() -> AuthPolicyConfig {
    AuthPolicyConfig {
        password_min_length: 8,
        lockout_failed_attempts: 5,
        lockout_minutes: 15,
        session_inactivity_minutes: 480,
        offline_only: true,
    }
}

fn test_retention_cfg() -> RetentionConfig {
    RetentionConfig {
        audit_log_days: 365,
        session_days: 30,
        patient_record_days: 2555,
        clinical_years_min: 7,
    }
}

fn stub_session() -> SessionRecord {
    SessionRecord {
        user_id: 1,
        username: "testuser".to_string(),
        role_name: "admin".to_string(),
        disabled: false,
        inactive_expired: false,
    }
}

fn build_app_state(repo: StubRepo) -> AppState {
    let pool = sqlx::mysql::MySqlPoolOptions::new()
        .connect_lazy_with(sqlx::mysql::MySqlConnectOptions::new());
    AppState {
        app_service: AppService::new(Arc::new(repo), &test_auth_policy(), &test_retention_cfg()),
        retention: RetentionSnapshot { audit_log_days: 365, session_days: 30, patient_record_days: 2555 },
        session: SessionSnapshot {
            cookie_name: "hospital_session".to_string(),
            secure: false,
            http_only: false,
            same_site: "Lax".to_string(),
            ttl_minutes: 480,
        },
        db_pool: pool,
    }
}

async fn build_client(repo: StubRepo) -> Client {
    use crate::infrastructure::auth::middleware::AuthFairing;
    use crate::routes::{auth, patients, retention};

    let rocket = rocket::build()
        .manage(build_app_state(repo))
        .attach(AuthFairing)
        .mount("/", rocket::routes![
            auth::login,
            auth::logout,
            retention::retention_settings,
            retention::retention_policies,
            retention::upsert_retention_policy,
            patients::search_patients,
        ]);

    Client::tracked(rocket).await.expect("valid rocket instance")
}

async fn build_client_full(repo: StubRepo) -> Client {
    use crate::infrastructure::auth::middleware::AuthFairing;
    use crate::routes::{auth, cafeteria, governance, ingestion, patients, retention};

    let rocket = rocket::build()
        .manage(build_app_state(repo))
        .attach(AuthFairing)
        .mount("/", rocket::routes![
            auth::login,
            auth::logout,
            retention::retention_settings,
            retention::retention_policies,
            retention::upsert_retention_policy,
            patients::search_patients,
            governance::create_record,
            governance::list_records,
            governance::tombstone_record,
            ingestion::list_tasks,
            ingestion::create_task,
            ingestion::task_versions,
            cafeteria::list_dishes,
            cafeteria::categories,
            cafeteria::upsert_rule,
            cafeteria::rules,
        ]);

    Client::tracked(rocket).await.expect("valid rocket instance")
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[rocket::async_test]
async fn unauth_get_protected_endpoint_returns_401() {
    let client = build_client(StubRepo::new()).await;
    let resp = client.get("/api/v1/retention/settings").dispatch().await;
    assert_eq!(resp.status(), Status::Unauthorized);
}

#[rocket::async_test]
async fn login_empty_username_returns_400() {
    let client = build_client(StubRepo::new()).await;
    let resp = client
        .post("/api/v1/auth/login")
        .header(rocket::http::ContentType::JSON)
        .body(r#"{"username":"","password":"Admin#Test1234"}"#)
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::BadRequest);
}

#[rocket::async_test]
async fn login_unknown_user_returns_401() {
    // StubRepo.user = None → no matching user in store
    let client = build_client(StubRepo::new()).await;
    let resp = client
        .post("/api/v1/auth/login")
        .header(rocket::http::ContentType::JSON)
        .body(r#"{"username":"ghost","password":"Admin#Test1234"}"#)
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Unauthorized);
}

#[rocket::async_test]
async fn login_valid_credentials_returns_200_and_64_char_csrf() {
    let hash = AppService::hash_password_argon2("Admin#Test1234").expect("argon2 hash");
    let user = UserAuthRecord {
        id: 1,
        username: "admin".to_string(),
        password_hash: hash,
        role_name: "admin".to_string(),
        disabled: false,
        failed_attempts: 0,
        locked_now: false,
    };
    let client = build_client(StubRepo::new().with_user(user)).await;
    let resp = client
        .post("/api/v1/auth/login")
        .header(rocket::http::ContentType::JSON)
        .body(r#"{"username":"admin","password":"Admin#Test1234"}"#)
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().await.expect("json body");
    let csrf = body["csrf_token"].as_str().expect("csrf_token field");
    assert_eq!(csrf.len(), 64, "csrf_token must be 64 hex characters");
    assert!(csrf.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(body["role"], "admin");
}

#[rocket::async_test]
async fn retention_settings_returns_snapshot_values_when_authenticated() {
    let client = build_client(StubRepo::new().with_session(stub_session())).await;
    let resp = client
        .get("/api/v1/retention/settings")
        .cookie(Cookie::new("hospital_session", "test_token"))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().await.expect("json body");
    assert_eq!(body["audit_log_days"], 365);
    assert_eq!(body["session_days"], 30);
    assert_eq!(body["patient_record_days"], 2555);
}

#[rocket::async_test]
async fn retention_policy_upsert_parses_path_params_and_returns_200() {
    let session_token = "test_session_token";
    let csrf = AppService::csrf_token_for(session_token);
    let client = build_client(
        StubRepo::new().with_session(stub_session()).with_permission(),
    )
    .await;
    let resp = client
        .put("/api/v1/retention/policies/audit_logs/7")
        .cookie(Cookie::new("hospital_session", session_token))
        .header(Header::new("X-CSRF-Token", csrf))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
}

#[rocket::async_test]
async fn retention_policy_upsert_without_csrf_returns_401() {
    let client = build_client(StubRepo::new().with_session(stub_session())).await;
    let resp = client
        .put("/api/v1/retention/policies/audit_logs/7")
        .cookie(Cookie::new("hospital_session", "test_token"))
        // No X-CSRF-Token header — fairing blocks mutating request
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Unauthorized);
}

#[rocket::async_test]
async fn patient_search_without_auth_returns_401() {
    let client = build_client(StubRepo::new()).await;
    let resp = client
        .get("/api/v1/patients/search?q=john")
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Unauthorized);
}

// ── Governance route tests ────────────────────────────────────────────────────

#[rocket::async_test]
async fn governance_list_unauthenticated_returns_401() {
    let client = build_client_full(StubRepo::new()).await;
    let resp = client.get("/api/v1/governance/records").dispatch().await;
    assert_eq!(resp.status(), Status::Unauthorized);
}

#[rocket::async_test]
async fn governance_list_authenticated_with_permission_returns_200() {
    let client = build_client_full(
        StubRepo::new().with_session(stub_session()).with_permission().with_reads_ok(),
    )
    .await;
    let resp = client
        .get("/api/v1/governance/records")
        .cookie(Cookie::new("hospital_session", "test_token"))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().await.expect("json array");
    assert!(body.is_array());
}

#[rocket::async_test]
async fn governance_create_without_csrf_returns_401() {
    let client = build_client_full(StubRepo::new().with_session(stub_session())).await;
    let resp = client
        .post("/api/v1/governance/records")
        .header(rocket::http::ContentType::JSON)
        .cookie(Cookie::new("hospital_session", "test_token"))
        // No X-CSRF-Token — fairing blocks all mutating requests
        .body(r#"{"tier":"raw","lineage_source_id":null,"lineage_metadata":"","payload_json":"{}"}"#)
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Unauthorized);
}

// ── Ingestion route tests ─────────────────────────────────────────────────────

#[rocket::async_test]
async fn ingestion_list_unauthenticated_returns_401() {
    let client = build_client_full(StubRepo::new()).await;
    let resp = client.get("/api/v1/ingestion/tasks").dispatch().await;
    assert_eq!(resp.status(), Status::Unauthorized);
}

#[rocket::async_test]
async fn ingestion_list_authenticated_with_permission_returns_200() {
    let client = build_client_full(
        StubRepo::new().with_session(stub_session()).with_permission().with_reads_ok(),
    )
    .await;
    let resp = client
        .get("/api/v1/ingestion/tasks")
        .cookie(Cookie::new("hospital_session", "test_token"))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().await.expect("json array");
    assert!(body.is_array());
}

// ── Cafeteria route tests ─────────────────────────────────────────────────────

#[rocket::async_test]
async fn cafeteria_dishes_unauthenticated_returns_401() {
    let client = build_client_full(StubRepo::new()).await;
    let resp = client.get("/api/v1/cafeteria/dishes").dispatch().await;
    assert_eq!(resp.status(), Status::Unauthorized);
}

#[rocket::async_test]
async fn cafeteria_dishes_authenticated_with_permission_returns_json_array() {
    let client = build_client_full(
        StubRepo::new().with_session(stub_session()).with_permission().with_reads_ok(),
    )
    .await;
    let resp = client
        .get("/api/v1/cafeteria/dishes")
        .cookie(Cookie::new("hospital_session", "test_token"))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Ok);
    let body: serde_json::Value = resp.into_json().await.expect("json array");
    assert!(body.is_array());
}

#[rocket::async_test]
async fn cafeteria_ranking_rule_upsert_without_csrf_returns_401() {
    let client = build_client_full(StubRepo::new().with_session(stub_session())).await;
    let resp = client
        .put("/api/v1/cafeteria/ranking-rules")
        .header(rocket::http::ContentType::JSON)
        .cookie(Cookie::new("hospital_session", "test_token"))
        // No X-CSRF-Token — fairing must block this mutating request
        .body(r#"{"rule_key":"popularity","weight":0.5,"enabled":true}"#)
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Unauthorized);
}

// ── Governance branch-logic and error-guard tests ─────────────────────────────

#[rocket::async_test]
async fn governance_create_invalid_tier_returns_400() {
    // AppService validates the tier string before reaching the repo — "bronze"
    // must be rejected with 400 even when auth and CSRF are both valid.
    let session_token = "test_session_token";
    let csrf = AppService::csrf_token_for(session_token);
    let client = build_client_full(
        StubRepo::new().with_session(stub_session()).with_permission().with_reads_ok(),
    )
    .await;
    let resp = client
        .post("/api/v1/governance/records")
        .header(rocket::http::ContentType::JSON)
        .cookie(Cookie::new("hospital_session", session_token))
        .header(Header::new("X-CSRF-Token", csrf))
        .body(r#"{"tier":"bronze","lineage_source_id":null,"lineage_metadata":"test","payload_json":"{}"}"#)
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::BadRequest);
}

#[rocket::async_test]
async fn governance_tombstone_without_csrf_returns_401() {
    let client = build_client_full(StubRepo::new().with_session(stub_session())).await;
    let resp = client
        .delete("/api/v1/governance/records/1")
        .header(rocket::http::ContentType::JSON)
        .cookie(Cookie::new("hospital_session", "test_token"))
        // No X-CSRF-Token — DELETE is a mutating verb; fairing must reject it
        .body(r#"{"reason":"test tombstone reason"}"#)
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Unauthorized);
}

#[rocket::async_test]
async fn governance_create_without_permission_returns_403() {
    // Valid CSRF + session but permission = false → AppService authorize() rejects
    let session_token = "test_session_token";
    let csrf = AppService::csrf_token_for(session_token);
    let client = build_client_full(
        // with_permission() intentionally omitted
        StubRepo::new().with_session(stub_session()),
    )
    .await;
    let resp = client
        .post("/api/v1/governance/records")
        .header(rocket::http::ContentType::JSON)
        .cookie(Cookie::new("hospital_session", session_token))
        .header(Header::new("X-CSRF-Token", csrf))
        .body(r#"{"tier":"raw","lineage_source_id":null,"lineage_metadata":"m","payload_json":"{}"}"#)
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Forbidden);
}

// ── Ingestion branch-logic and error-guard tests ──────────────────────────────

#[rocket::async_test]
async fn ingestion_create_without_csrf_returns_401() {
    let client = build_client_full(StubRepo::new().with_session(stub_session())).await;
    let resp = client
        .post("/api/v1/ingestion/tasks")
        .header(rocket::http::ContentType::JSON)
        .cookie(Cookie::new("hospital_session", "test_token"))
        // No X-CSRF-Token
        .body(r#"{"task_name":"t","seed_urls":[],"extraction_rules_json":"{}","pagination_strategy":"breadth-first","max_depth":1,"schedule_cron":"0 * * * *"}"#)
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Unauthorized);
}

#[rocket::async_test]
async fn ingestion_task_versions_without_auth_returns_401() {
    let client = build_client_full(StubRepo::new()).await;
    let resp = client
        .get("/api/v1/ingestion/tasks/1/versions")
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Unauthorized);
}

#[rocket::async_test]
async fn ingestion_list_without_permission_returns_403() {
    // Session present but no ingestion.read permission
    let client = build_client_full(
        // with_permission() omitted
        StubRepo::new().with_session(stub_session()),
    )
    .await;
    let resp = client
        .get("/api/v1/ingestion/tasks")
        .cookie(Cookie::new("hospital_session", "test_token"))
        .dispatch()
        .await;
    assert_eq!(resp.status(), Status::Forbidden);
}
