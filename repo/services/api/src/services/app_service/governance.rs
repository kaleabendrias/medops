use reqwest::Url as ReqwestUrl;
use contracts::{
    AuditLogDto, ExperimentAssignRequest, ExperimentBacktrackRequest, ExperimentCreateRequest,
    ExperimentVariantRequest, FunnelMetricsDto, GovernanceDeleteRequest, GovernanceRecordDto,
    GovernanceRecordRequest, IngestionTaskCreateRequest, IngestionTaskDto,
    IngestionTaskRollbackRequest, IngestionTaskRunDto, IngestionTaskUpdateRequest,
    IngestionTaskVersionDto, RecommendationKpiDto, RetentionMetricsDto, RetentionPolicyDto,
    TelemetryEventRequest,
};

use crate::contracts::{ApiError, AuthUser};
use super::AppService;

impl AppService {
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

    fn is_allowed_seed_url(url: &str) -> bool {
        if let Some(path) = url.strip_prefix("file://") {
            return path.starts_with("/var/lib/hospital-ingest/")
                || path.starts_with("/srv/ingest/")
                || path.starts_with("/app/config/ingestion_fixture/");
        }
        if let Ok(parsed) = ReqwestUrl::parse(url) {
            if parsed.scheme() != "http" && parsed.scheme() != "https" {
                return false;
            }
            if let Some(host) = parsed.host_str() {
                return host == "localhost"
                    || host == "127.0.0.1"
                    || host == "api"
                    || host == "mysql"
                    || host.ends_with(".local")
                    || host.ends_with(".internal");
            }
        }
        false
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
        for url in &req.seed_urls {
            if !Self::is_allowed_seed_url(url.trim()) {
                return Err(ApiError::bad_request(&format!(
                    "Seed URL is not on the intranet allowlist: {url}"
                )));
            }
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
        for url in &req.seed_urls {
            if !Self::is_allowed_seed_url(url.trim()) {
                return Err(ApiError::bad_request(&format!(
                    "Seed URL is not on the intranet allowlist: {url}"
                )));
            }
        }
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
}
