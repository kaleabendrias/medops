use std::collections::{HashSet, VecDeque};
use std::str::FromStr;

use async_trait::async_trait;
use chrono::{SecondsFormat, Utc};
use cron::Schedule;
use regex::Regex;
use scraper::{Html, Selector};
use sha2::{Digest, Sha256};
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
use crate::infrastructure::security::field_crypto::FieldCrypto;
use crate::repositories::app_repository::{
    AppRepository, AttachmentStorageRecord, BedTransitionDbRequest, OrderRecord,
    PatientSensitiveRecord, SessionRecord, UserAuthRecord,
};

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
    /// Backed by the `role_permissions` table — see migration 021 for the
    /// seed that grants `patient.global_access` to admin/auditor.
    async fn has_global_patient_access(&self, role_name: &str) -> Result<bool, ApiError> {
        self.user_has_permission(role_name, "patient.global_access").await
    }

    /// Capability check for "this role can see/operate on dining orders
    /// created by any user". Backed by `order.global_access` in
    /// `role_permissions` (admin / auditor / employee, per migration 010
    /// and 021).
    async fn has_global_order_access(&self, role_name: &str) -> Result<bool, ApiError> {
        self.user_has_permission(role_name, "order.global_access").await
    }

    /// Capability check for "this role can see/operate on ingestion tasks
    /// created by any user". Backed by `ingestion.global_access` in
    /// `role_permissions` (admin / auditor, per migration 021).
    async fn has_global_ingestion_access(&self, role_name: &str) -> Result<bool, ApiError> {
        self.user_has_permission(role_name, "ingestion.global_access").await
    }

    /// Stateless bed state-machine validation. Re-used by both the service
    /// layer (for early-fail UX) and the repository layer (re-checked
    /// inside the atomic transition transaction so concurrent state changes
    /// can never sneak past).
    fn validate_bed_state_transition(current: &str, target: &str) -> Result<(), ApiError> {
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

    fn next_run_iso(schedule_cron: &str) -> Result<String, ApiError> {
        let raw = schedule_cron.trim();
        let normalized = if raw.split_whitespace().count() == 5 {
            format!("0 {raw}")
        } else {
            raw.to_string()
        };
        let schedule = Schedule::from_str(&normalized)
            .map_err(|_| ApiError::bad_request("Invalid schedule_cron expression"))?;
        let now = Utc::now();
        let next = schedule
            .after(&now)
            .next()
            .ok_or_else(|| ApiError::bad_request("schedule_cron does not produce a future run"))?;
        Ok(next.to_rfc3339_opts(SecondsFormat::Secs, true))
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

    fn to_masked_patient(row: PatientSensitiveRecord) -> PatientProfileDto {
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

    fn parse_extraction_config(rules_json: &str) -> Result<(String, Vec<String>, Option<String>), ApiError> {
        let value: serde_json::Value = serde_json::from_str(rules_json)
            .map_err(|_| ApiError::bad_request("Invalid extraction_rules_json"))?;
        let mode = value
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("regex")
            .to_string();

        let mut fields = value
            .get("fields")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        if fields.is_empty() {
            if let Some(single) = value.get("selector").and_then(|v| v.as_str()) {
                fields.push(single.to_string());
            }
            if let Some(single) = value.get("pattern").and_then(|v| v.as_str()) {
                fields.push(single.to_string());
            }
        }

        if fields.is_empty() {
            return Err(ApiError::bad_request("Extraction rules must include fields/selector/pattern"));
        }

        let pagination_selector = value
            .get("pagination_selector")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok((mode, fields, pagination_selector))
    }

    /// Approved internal hostnames for intranet ingestion fetches.
    const INTRANET_ALLOWED_HOSTS: &'static [&'static str] = &[
        "localhost",
        "127.0.0.1",
        "::1",
        "api",
        "mysql",
        "web",
    ];

    /// Approved base directories for file:// ingestion sources.
    const FILE_ALLOWED_BASES: &'static [&'static str] = &[
        "/app/config/ingestion_fixture",
        "/var/lib/rocket-api/ingestion",
    ];

    /// Synchronous, syntactic-only sanity check on a file:// path. This
    /// rejects the obvious classes of bad input — relative paths, ParentDir
    /// segments, sibling-prefix bypasses against the configured base
    /// directories — but it does NOT touch the filesystem and therefore
    /// cannot detect symlink escapes. The strong, IO-backed check lives in
    /// `canonical_allowed_file_path` and runs before the file is opened.
    fn is_syntactic_allowed_file_path(path: &str) -> bool {
        let p = std::path::Path::new(path);
        if !p.is_absolute() {
            return false;
        }
        for component in p.components() {
            if matches!(component, std::path::Component::ParentDir) {
                return false;
            }
        }
        // `Path::starts_with` is COMPONENT-AWARE, so the
        // sibling-prefix attack
        //   /app/config/ingestion_fixture_secret/x.json
        // does NOT match
        //   /app/config/ingestion_fixture
        // unlike a raw `str::starts_with` test.
        Self::FILE_ALLOWED_BASES
            .iter()
            .any(|base| p.starts_with(std::path::Path::new(base)))
    }

    /// Strong, IO-backed validation of a file:// path: canonicalize the
    /// requested path AND each authorized base directory, then verify that
    /// the resolved path lives inside one of the resolved bases. Because
    /// `tokio::fs::canonicalize` follows symlinks, this defeats both
    /// path-traversal and symlink-escape attacks (e.g. a fixture file that
    /// is actually a symlink to /etc/passwd will canonicalize to
    /// /etc/passwd and be rejected by the boundary check).
    async fn canonical_allowed_file_path(
        path: &str,
    ) -> Result<std::path::PathBuf, ApiError> {
        if !Self::is_syntactic_allowed_file_path(path) {
            return Err(ApiError::bad_request(
                "Seed file path is outside the approved ingestion directories",
            ));
        }
        let resolved = tokio::fs::canonicalize(path).await.map_err(|_| {
            ApiError::bad_request("Seed file does not exist or is not accessible")
        })?;
        let mut resolved_bases: Vec<std::path::PathBuf> = Vec::new();
        for base in Self::FILE_ALLOWED_BASES {
            if let Ok(c) = tokio::fs::canonicalize(base).await {
                resolved_bases.push(c);
            }
        }
        if resolved_bases.is_empty() {
            return Err(ApiError::bad_request(
                "No ingestion source directories are configured on this host",
            ));
        }
        let inside = resolved_bases.iter().any(|b| resolved.starts_with(b));
        if !inside {
            return Err(ApiError::bad_request(
                "Resolved seed file path escapes approved ingestion directories",
            ));
        }
        Ok(resolved)
    }

    fn is_allowed_intranet_url(url: &str) -> bool {
        if let Some(path) = url.strip_prefix("file://") {
            // Cheap syntactic gate; the strong IO-backed check is enforced
            // at the actual fetch boundary in `fetch_source`, which is the
            // only place a file is ever opened.
            return Self::is_syntactic_allowed_file_path(path);
        }
        if let Ok(parsed) = reqwest::Url::parse(url) {
            if parsed.scheme() != "http" && parsed.scheme() != "https" {
                return false;
            }
            if let Some(host) = parsed.host_str() {
                return Self::INTRANET_ALLOWED_HOSTS.iter().any(|&h| h == host)
                    || host.ends_with(".local")
                    || host.ends_with(".internal");
            }
        }
        false
    }

    async fn fetch_source(url: &str) -> Result<String, ApiError> {
        if let Some(path) = url.strip_prefix("file://") {
            // STRONG, IO-backed boundary check before any file is opened.
            // This canonicalizes both the requested path and the configured
            // base directories and rejects symlink escapes, sibling-prefix
            // bypasses, and traversal sequences.
            let canonical = Self::canonical_allowed_file_path(path).await?;
            return tokio::fs::read_to_string(canonical).await.map_err(|_| {
                ApiError::bad_request("Unable to read seed URL file")
            });
        }

        if !Self::is_allowed_intranet_url(url) {
            return Err(ApiError::bad_request(
                "Seed URL must use file:// within approved directories or target an approved intranet host (localhost, *.local, *.internal, or Docker service names). Public internet URLs and path traversal are not permitted.",
            ));
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(8))
            .build()
            .map_err(|_| ApiError::Internal)?;

        let response = client
            .get(url)
            .send()
            .await
            .map_err(|_| ApiError::bad_request("Unable to fetch seed URL"))?;
        let status = response.status();
        if !status.is_success() {
            return Err(ApiError::bad_request("Seed URL returned non-success status"));
        }
        response
            .text()
            .await
            .map_err(|_| ApiError::bad_request("Unable to decode seed URL response"))
    }

    fn normalize_next_url(base_url: &str, href: &str) -> Option<String> {
        if href.starts_with("http://") || href.starts_with("https://") || href.starts_with("file://") {
            let candidate = href.to_string();
            return if Self::is_allowed_intranet_url(&candidate) { Some(candidate) } else { None };
        }

        if let Some(base_path) = base_url.strip_prefix("file://") {
            let parent = std::path::Path::new(base_path).parent()?;
            let joined = parent.join(href);
            return Some(format!("file://{}", joined.to_string_lossy()));
        }

        if let Ok(parsed) = reqwest::Url::parse(base_url) {
            if let Ok(next) = parsed.join(href) {
                let candidate = next.to_string();
                return if Self::is_allowed_intranet_url(&candidate) { Some(candidate) } else { None };
            }
        }

        None
    }

    fn xpath_to_css(expr: &str) -> Option<String> {
        let trimmed = expr.trim();
        if !trimmed.starts_with("//") {
            return None;
        }
        let inner = &trimmed[2..];
        if let Some((tag, rest)) = inner.split_once("[@id='") {
            let id = rest.strip_suffix("']")?;
            return Some(format!("{}#{}", tag, id));
        }
        if let Some((tag, rest)) = inner.split_once("[@class='") {
            let class = rest.strip_suffix("']")?;
            return Some(format!("{}.{}", tag, class.replace(' ', ".")));
        }
        Some(inner.to_string())
    }

    fn extract_jsonpath(content: &str, jsonpath: &str) -> Vec<String> {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(content) else {
            return Vec::new();
        };

        let path = jsonpath.trim();
        if !path.starts_with("$") {
            return Vec::new();
        }

        if let Some((left, right)) = path.split_once("[*].") {
            let key = left.trim_start_matches("$.");
            let field = right;
            let Some(arr) = value.get(key).and_then(|v| v.as_array()) else {
                return Vec::new();
            };
            return arr
                .iter()
                .filter_map(|item| item.get(field))
                .map(|v| v.as_str().map_or_else(|| v.to_string(), ToString::to_string))
                .collect();
        }

        let mut cursor = &value;
        for segment in path.trim_start_matches("$.").split('.') {
            if segment.is_empty() {
                continue;
            }
            let Some(next) = cursor.get(segment) else {
                return Vec::new();
            };
            cursor = next;
        }
        vec![cursor
            .as_str()
            .map_or_else(|| cursor.to_string(), ToString::to_string)]
    }

    fn extract_records(mode: &str, fields: &[String], content: &str, source_url: &str) -> Result<Vec<serde_json::Value>, ApiError> {
        let mut out = Vec::new();
        match mode {
            "css" => {
                let doc = Html::parse_document(content);
                for rule in fields {
                    let selector = Selector::parse(rule)
                        .map_err(|_| ApiError::bad_request("Invalid CSS selector in extraction rules"))?;
                    for node in doc.select(&selector) {
                        let value = node.text().collect::<Vec<_>>().join(" ").trim().to_string();
                        if value.is_empty() {
                            continue;
                        }
                        out.push(serde_json::json!({"source_url": source_url, "rule": rule, "value": value}));
                    }
                }
            }
            "xpath" => {
                let doc = Html::parse_document(content);
                for rule in fields {
                    let css = Self::xpath_to_css(rule)
                        .ok_or_else(|| ApiError::bad_request("Unsupported XPath expression"))?;
                    let selector = Selector::parse(&css)
                        .map_err(|_| ApiError::bad_request("Invalid XPath expression"))?;
                    for node in doc.select(&selector) {
                        let value = node.text().collect::<Vec<_>>().join(" ").trim().to_string();
                        if value.is_empty() {
                            continue;
                        }
                        out.push(serde_json::json!({"source_url": source_url, "rule": rule, "value": value}));
                    }
                }
            }
            "jsonpath" => {
                for rule in fields {
                    for value in Self::extract_jsonpath(content, rule) {
                        out.push(serde_json::json!({"source_url": source_url, "rule": rule, "value": value}));
                    }
                }
            }
            "regex" => {
                for rule in fields {
                    let re = Regex::new(rule)
                        .map_err(|_| ApiError::bad_request("Invalid regex pattern in extraction rules"))?;
                    for cap in re.captures_iter(content) {
                        let value = cap
                            .get(1)
                            .or_else(|| cap.get(0))
                            .map(|m| m.as_str().trim().to_string())
                            .unwrap_or_default();
                        if value.is_empty() {
                            continue;
                        }
                        out.push(serde_json::json!({"source_url": source_url, "rule": rule, "value": value}));
                    }
                }
            }
            _ => return Err(ApiError::bad_request("Unsupported extraction mode")),
        }

        Ok(out)
    }

    fn discover_links(base_url: &str, content: &str, pagination_selector: Option<&str>) -> Vec<String> {
        let mut links = Vec::new();
        let doc = Html::parse_document(content);
        let selector = pagination_selector
            .and_then(|s| Selector::parse(s).ok())
            .or_else(|| Selector::parse("a[href]").ok());

        if let Some(sel) = selector {
            for node in doc.select(&sel) {
                if let Some(href) = node.value().attr("href") {
                    if let Some(next) = Self::normalize_next_url(base_url, href) {
                        links.push(next);
                    }
                }
            }
        }
        links
    }

}

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

    async fn can_access_patient(&self, user_id: i64, role_name: &str, patient_id: i64) -> Result<bool, ApiError> {
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

    async fn assign_patient(&self, patient_id: i64, target_user_id: i64, assignment_type: &str, assigned_by: i64) -> Result<(), ApiError> {
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

    async fn get_patient(&self, patient_id: i64) -> Result<Option<PatientProfileDto>, ApiError> {
        let row = self.get_patient_sensitive(patient_id).await?;
        Ok(row.map(Self::to_masked_patient))
    }

    async fn get_patient_sensitive(&self, patient_id: i64) -> Result<Option<PatientSensitiveRecord>, ApiError> {
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

    async fn list_patients(&self, user_id: i64, role_name: &str, limit: i64, offset: i64) -> Result<Vec<PatientProfileDto>, ApiError> {
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

    async fn update_patient_clinical_field(&self, patient_id: i64, field_name: &str, value: &str) -> Result<(), ApiError> {
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

    async fn add_patient_visit_note(&self, patient_id: i64, note: &str, actor_id: i64) -> Result<(), ApiError> {
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

    async fn list_patient_revisions(&self, patient_id: i64) -> Result<Vec<RevisionTimelineDto>, ApiError> {
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

    async fn create_patient_revision(
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

    async fn create_attachment_metadata(
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

    async fn list_attachments(&self, patient_id: i64) -> Result<Vec<AttachmentMetadataDto>, ApiError> {
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

    async fn get_attachment_storage(
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

    async fn list_beds(&self) -> Result<Vec<BedDto>, ApiError> {
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

    async fn get_bed_state(&self, bed_id: i64) -> Result<Option<String>, ApiError> {
        let state = sqlx::query_scalar::<_, String>("SELECT state FROM beds WHERE id = ?")
            .bind(bed_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(state)
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

    async fn apply_bed_transition(
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

    async fn list_bed_events(&self) -> Result<Vec<BedEventDto>, ApiError> {
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

    async fn create_menu(&self, menu_date: &str, meal_period: &str, item_name: &str, calories: i32, actor_id: i64) -> Result<(), ApiError> {
        sqlx::query(
            "INSERT INTO dining_menus (menu_date, meal_period, item_name, calories, created_by, created_at)
             VALUES (?, ?, ?, ?, ?, NOW())",
        )
        .bind(menu_date)
        .bind(meal_period)
        .bind(item_name)
        .bind(calories)
        .bind(actor_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn list_menus(&self) -> Result<Vec<DiningMenuDto>, ApiError> {
        let rows = sqlx::query_as::<_, (i64, String, String, String, i32)>(
            "SELECT id, menu_date, meal_period, item_name, calories FROM dining_menus ORDER BY menu_date DESC, id DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| DiningMenuDto {
                id: r.0,
                menu_date: r.1,
                meal_period: r.2,
                item_name: r.3,
                calories: r.4,
            })
            .collect())
    }

    async fn validate_menu_orderable(&self, menu_id: i64) -> Result<(), ApiError> {
        // Pre-flight menu governance enforcement for the order creation flow.
        //
        // We treat `dining_menus` as the "menu line" and the matching `dishes`
        // row (joined by name) as the governing dish record. The order is
        // only allowed to be committed when ALL of the following hold:
        //
        //   1. The menu line exists.
        //   2. A `dishes` row is actively linked to the menu line by name.
        //   3. dishes.is_published = TRUE.
        //   4. dishes.is_sold_out  = FALSE.
        //   5. The current server clock-time falls inside at least one
        //      configured `dish_sales_windows` row for that dish.
        //
        // Each violation maps to an explicit 400/403 so the frontend can
        // surface a precise reason and so abuse attempts (e.g. ordering a
        // sold-out item via a hand-crafted curl) are blocked at the service
        // boundary rather than relying on database integrity to fail late.

        // Step 1 — does the menu line exist at all?
        let menu_row = sqlx::query_as::<_, (String,)>(
            "SELECT item_name FROM dining_menus WHERE id = ?",
        )
        .bind(menu_id)
        .fetch_optional(&self.pool)
        .await?;
        let item_name = match menu_row {
            Some((name,)) => name,
            None => return Err(ApiError::bad_request("Menu line does not exist")),
        };

        // Step 2 — find the governing dish row (linked by exact name match).
        let dish_row = sqlx::query_as::<_, (i64, bool, bool)>(
            "SELECT id, is_published, is_sold_out FROM dishes WHERE name = ? LIMIT 1",
        )
        .bind(&item_name)
        .fetch_optional(&self.pool)
        .await?;
        let (dish_id, is_published, is_sold_out) = match dish_row {
            Some(v) => v,
            None => {
                return Err(ApiError::bad_request(
                    "Menu item is not linked to an active dish",
                ));
            }
        };

        // Step 3 — must be published.
        if !is_published {
            return Err(ApiError::Forbidden);
        }

        // Step 4 — must not be sold out.
        if is_sold_out {
            return Err(ApiError::Forbidden);
        }

        // Step 5 — must be inside at least one configured sales window.
        // dish_sales_windows stores start_hhmm/end_hhmm as zero-padded
        // 'HH:MM'; the lexicographic BETWEEN comparison is therefore
        // correct, and inclusive on both ends so a window of
        // '00:00'..'23:59' represents an "always on" SKU.
        let in_window = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(1) FROM dish_sales_windows
             WHERE dish_id = ?
               AND DATE_FORMAT(NOW(), '%H:%i') BETWEEN start_hhmm AND end_hhmm",
        )
        .bind(dish_id)
        .fetch_one(&self.pool)
        .await?;
        if in_window == 0 {
            return Err(ApiError::Forbidden);
        }

        Ok(())
    }

    async fn create_order(&self, patient_id: i64, menu_id: i64, notes: &str, actor_id: i64) -> Result<i64, ApiError> {
        self.create_order_idempotent(patient_id, menu_id, notes, actor_id, None)
            .await
    }

    async fn create_order_idempotent(
        &self,
        patient_id: i64,
        menu_id: i64,
        notes: &str,
        actor_id: i64,
        idempotency_key: Option<&str>,
    ) -> Result<i64, ApiError> {
        if let Some(key) = idempotency_key {
            let existing = sqlx::query_scalar::<_, i64>(
                "SELECT id FROM dining_orders WHERE idempotency_key = ? AND created_by = ?",
            )
            .bind(key)
            .bind(actor_id)
            .fetch_optional(&self.pool)
            .await?;
            if let Some(order_id) = existing {
                return Ok(order_id);
            }
        }

        let result = sqlx::query(
            "INSERT INTO dining_orders (patient_id, menu_id, status, notes, created_by, idempotency_key, created_at)
             VALUES (?, ?, 'Created', ?, ?, ?, NOW())",
        )
        .bind(patient_id)
        .bind(menu_id)
        .bind(notes)
        .bind(actor_id)
        .bind(idempotency_key)
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "UPDATE group_campaigns gc
             JOIN campaign_members cm ON cm.campaign_id = gc.id AND cm.user_id = ?
             JOIN dishes d ON d.id = gc.dish_id
             JOIN dining_menus dm ON dm.id = ? AND dm.item_name = d.name
             SET gc.last_activity_at = NOW()
             WHERE gc.status = 'Open'",
        )
        .bind(actor_id)
        .bind(menu_id)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_id() as i64)
    }

    async fn get_order(&self, order_id: i64) -> Result<Option<OrderRecord>, ApiError> {
        let row = sqlx::query_as::<_, (i64, i64, i64, String, String, i32, i64)>(
            "SELECT id, patient_id, menu_id, status, notes, version, created_by FROM dining_orders WHERE id = ?",
        )
        .bind(order_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| OrderRecord {
            id: r.0,
            patient_id: r.1,
            menu_id: r.2,
            status: r.3,
            notes: r.4,
            version: r.5,
            created_by: r.6,
        }))
    }

    async fn set_order_status_if_version(
        &self,
        order_id: i64,
        expected_version: i32,
        next_status: &str,
        reason: Option<&str>,
    ) -> Result<bool, ApiError> {
        let updated = sqlx::query(
            "UPDATE dining_orders
             SET status = ?,
                 status_reason = ?,
                 billed_at = CASE WHEN ? = 'Billed' THEN NOW() ELSE billed_at END,
                 canceled_at = CASE WHEN ? = 'Canceled' THEN NOW() ELSE canceled_at END,
                 credited_at = CASE WHEN ? = 'Credited' THEN NOW() ELSE credited_at END,
                 version = version + 1
             WHERE id = ? AND version = ?",
        )
        .bind(next_status)
        .bind(reason)
        .bind(next_status)
        .bind(next_status)
        .bind(next_status)
        .bind(order_id)
        .bind(expected_version)
        .execute(&self.pool)
        .await?
        .rows_affected();
        Ok(updated > 0)
    }

    async fn list_orders(&self, user_id: i64, role_name: &str, limit: i64, offset: i64) -> Result<Vec<OrderDto>, ApiError> {
        let safe_limit = limit.clamp(1, 200);
        let safe_offset = offset.max(0);
        let has_self_service = self
            .user_has_permission(role_name, "order.self_service")
            .await?;
        let rows = if self.has_global_order_access(role_name).await? {
            sqlx::query_as::<_, (i64, i64, i64, String, String, i32)>(
                "SELECT id, patient_id, menu_id, status, notes, version FROM dining_orders ORDER BY id DESC LIMIT ? OFFSET ?",
            )
            .bind(safe_limit)
            .bind(safe_offset)
            .fetch_all(&self.pool)
            .await?
        } else if has_self_service {
            // Self-service users only see orders they created themselves.
            sqlx::query_as::<_, (i64, i64, i64, String, String, i32)>(
                "SELECT id, patient_id, menu_id, status, notes, version
                 FROM dining_orders
                 WHERE created_by = ?
                 ORDER BY id DESC LIMIT ? OFFSET ?",
            )
            .bind(user_id)
            .bind(safe_limit)
            .bind(safe_offset)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, (i64, i64, i64, String, String, i32)>(
                "SELECT o.id, o.patient_id, o.menu_id, o.status, o.notes, o.version
                 FROM dining_orders o
                 JOIN patient_assignments pa ON pa.patient_id = o.patient_id AND pa.user_id = ?
                 ORDER BY o.id DESC LIMIT ? OFFSET ?",
            )
            .bind(user_id)
            .bind(safe_limit)
            .bind(safe_offset)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows
            .into_iter()
            .map(|r| OrderDto {
                id: r.0,
                patient_id: r.1,
                menu_id: r.2,
                status: r.3,
                notes: r.4,
                version: r.5,
            })
            .collect())
    }

    async fn create_governance_record(
        &self,
        tier: &str,
        lineage_source_id: Option<i64>,
        lineage_metadata: &str,
        payload_json: &str,
        actor_id: i64,
    ) -> Result<i64, ApiError> {
        // Three distinct physical tables: governance_raw, governance_cleaned,
        // governance_analytics. Each tier above raw REQUIRES a lineage_source_id
        // pointing at the immediately lower tier; the database FKs reject any
        // value that doesn't reference a real row in that lower tier.
        //
        // To keep ids globally unique across the three tables (so the public
        // /governance/records/{id} surface stays unambiguous), every insert
        // first allocates an id from the shared `governance_id_sequence`
        // table and then stores it as the PK of the chosen tier table.
        let mut tx = self.pool.begin().await.map_err(|_| ApiError::Internal)?;

        let allocated = sqlx::query("INSERT INTO governance_id_sequence () VALUES ()")
            .execute(&mut *tx)
            .await?;
        let new_id = allocated.last_insert_id() as i64;

        match tier {
            "raw" => {
                if lineage_source_id.is_some() {
                    return Err(ApiError::bad_request(
                        "raw governance records must not declare a lineage source",
                    ));
                }
                sqlx::query(
                    "INSERT INTO governance_raw
                     (id, lineage_metadata, payload_json, tombstoned, tombstone_reason, created_by, created_at)
                     VALUES (?, ?, ?, FALSE, NULL, ?, NOW())",
                )
                .bind(new_id)
                .bind(lineage_metadata)
                .bind(payload_json)
                .bind(actor_id)
                .execute(&mut *tx)
                .await?;
            }
            "cleaned" => {
                let source = lineage_source_id.ok_or_else(|| {
                    ApiError::bad_request(
                        "cleaned governance records require a lineage source from governance_raw",
                    )
                })?;
                sqlx::query(
                    "INSERT INTO governance_cleaned
                     (id, lineage_source_id, lineage_metadata, payload_json, tombstoned, tombstone_reason, created_by, created_at)
                     VALUES (?, ?, ?, ?, FALSE, NULL, ?, NOW())",
                )
                .bind(new_id)
                .bind(source)
                .bind(lineage_metadata)
                .bind(payload_json)
                .bind(actor_id)
                .execute(&mut *tx)
                .await
                .map_err(|_| {
                    ApiError::bad_request(
                        "lineage_source_id does not reference an existing governance_raw row",
                    )
                })?;
            }
            "analytics" => {
                let source = lineage_source_id.ok_or_else(|| {
                    ApiError::bad_request(
                        "analytics governance records require a lineage source from governance_cleaned",
                    )
                })?;
                sqlx::query(
                    "INSERT INTO governance_analytics
                     (id, lineage_source_id, lineage_metadata, payload_json, tombstoned, tombstone_reason, created_by, created_at)
                     VALUES (?, ?, ?, ?, FALSE, NULL, ?, NOW())",
                )
                .bind(new_id)
                .bind(source)
                .bind(lineage_metadata)
                .bind(payload_json)
                .bind(actor_id)
                .execute(&mut *tx)
                .await
                .map_err(|_| {
                    ApiError::bad_request(
                        "lineage_source_id does not reference an existing governance_cleaned row",
                    )
                })?;
            }
            _ => {
                return Err(ApiError::bad_request(
                    "tier must be raw, cleaned, or analytics",
                ));
            }
        }

        tx.commit().await.map_err(|_| ApiError::Internal)?;
        Ok(new_id)
    }

    async fn list_governance_records(&self) -> Result<Vec<GovernanceRecordDto>, ApiError> {
        // UNION ALL across the three physical tier tables. Each branch
        // synthesizes the `tier` discriminator and aligns the column shape
        // (raw rows have no lineage_source_id, hence the NULL cast).
        let rows = sqlx::query_as::<_, (i64, String, Option<i64>, String, String, bool)>(
            "SELECT id, 'raw' AS tier, CAST(NULL AS SIGNED) AS lineage_source_id,
                    lineage_metadata, payload_json, tombstoned
               FROM governance_raw
             UNION ALL
             SELECT id, 'cleaned' AS tier, lineage_source_id,
                    lineage_metadata, payload_json, tombstoned
               FROM governance_cleaned
             UNION ALL
             SELECT id, 'analytics' AS tier, lineage_source_id,
                    lineage_metadata, payload_json, tombstoned
               FROM governance_analytics
             ORDER BY id DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| GovernanceRecordDto {
                id: r.0,
                tier: r.1,
                lineage_source_id: r.2,
                lineage_metadata: r.3,
                payload_json: r.4,
                tombstoned: r.5,
            })
            .collect())
    }

    async fn tombstone_governance_record(&self, record_id: i64, reason: &str) -> Result<(), ApiError> {
        // Ids are globally unique across the three tier tables (allocated
        // from `governance_id_sequence`), so we just attempt the UPDATE in
        // each table and stop at the first one that hits a row.
        let raw = sqlx::query(
            "UPDATE governance_raw SET tombstoned = TRUE, tombstone_reason = ? WHERE id = ?",
        )
        .bind(reason)
        .bind(record_id)
        .execute(&self.pool)
        .await?
        .rows_affected();
        if raw > 0 {
            return Ok(());
        }

        let cleaned = sqlx::query(
            "UPDATE governance_cleaned SET tombstoned = TRUE, tombstone_reason = ? WHERE id = ?",
        )
        .bind(reason)
        .bind(record_id)
        .execute(&self.pool)
        .await?
        .rows_affected();
        if cleaned > 0 {
            return Ok(());
        }

        let analytics = sqlx::query(
            "UPDATE governance_analytics SET tombstoned = TRUE, tombstone_reason = ? WHERE id = ?",
        )
        .bind(reason)
        .bind(record_id)
        .execute(&self.pool)
        .await?
        .rows_affected();
        if analytics == 0 {
            return Err(ApiError::NotFound);
        }
        Ok(())
    }

    async fn create_telemetry_event(&self, experiment_key: &str, user_id: i64, event_name: &str, payload_json: &str) -> Result<(), ApiError> {
        sqlx::query(
            "INSERT INTO telemetry_events (experiment_id, user_id, event_name, payload_json, created_at)
             SELECT e.id, ?, ?, ?, NOW() FROM experiments e WHERE e.experiment_key = ? AND e.is_active = TRUE",
        )
        .bind(user_id)
        .bind(event_name)
        .bind(payload_json)
        .bind(experiment_key)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn append_audit(
        &self,
        action_type: &str,
        entity_type: &str,
        entity_id: &str,
        details_json: &str,
        actor_id: i64,
    ) -> Result<(), ApiError> {
        // Use a serialized transaction to prevent concurrent callers from
        // reading the same MAX(event_seq) and producing duplicate sequence
        // numbers or a broken hash chain.
        let mut tx = self.pool.begin().await.map_err(|_| ApiError::Internal)?;

        // Lock the latest row to serialize concurrent appends.
        let last = sqlx::query_as::<_, (Option<i64>, Option<String>)>(
            "SELECT event_seq, entry_hash FROM audit_logs ORDER BY event_seq DESC LIMIT 1 FOR UPDATE",
        )
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or((None, None));

        let next_seq = last.0.unwrap_or(0) + 1;
        let prev_hash = last.1.unwrap_or_default();
        let payload = format!(
            "{}|{}|{}|{}|{}|{}",
            next_seq, action_type, entity_type, entity_id, details_json, actor_id
        );
        let mut hasher = Sha256::new();
        hasher.update(prev_hash.as_bytes());
        hasher.update(payload.as_bytes());
        let entry_hash = hex::encode(hasher.finalize());

        sqlx::query(
            "INSERT INTO audit_logs (event_seq, action_type, entity_type, entity_id, details_json, actor_id, prev_hash, entry_hash, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, NOW())",
        )
        .bind(next_seq)
        .bind(action_type)
        .bind(entity_type)
        .bind(entity_id)
        .bind(details_json)
        .bind(actor_id)
        .bind(&prev_hash)
        .bind(&entry_hash)
        .execute(&mut *tx)
        .await?;

        tx.commit().await.map_err(|_| ApiError::Internal)?;
        Ok(())
    }

    async fn list_audits(&self) -> Result<Vec<AuditLogDto>, ApiError> {
        let rows = sqlx::query_as::<_, (i64, String, String, String, String, String)>(
            "SELECT al.id, al.action_type, al.entity_type, al.entity_id, u.username,
             DATE_FORMAT(al.created_at, '%Y-%m-%d %H:%i:%s')
             FROM audit_logs al
             JOIN users u ON u.id = al.actor_id
             ORDER BY al.id DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| AuditLogDto {
                id: r.0,
                action_type: r.1,
                entity_type: r.2,
                entity_id: r.3,
                actor_username: r.4,
                created_at: r.5,
            })
            .collect())
    }

    async fn list_retention_policies(&self) -> Result<Vec<RetentionPolicyDto>, ApiError> {
        let rows = sqlx::query_as::<_, (String, i32)>("SELECT policy_key, years FROM retention_policies ORDER BY policy_key")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(|(policy_key, years)| RetentionPolicyDto { policy_key, years })
            .collect())
    }

    async fn upsert_retention_policy(&self, policy_key: &str, years: i32, actor_id: i64) -> Result<(), ApiError> {
        sqlx::query(
            "INSERT INTO retention_policies (policy_key, years, updated_by, updated_at)
             VALUES (?, ?, ?, NOW())
             ON DUPLICATE KEY UPDATE years = VALUES(years), updated_by = VALUES(updated_by), updated_at = VALUES(updated_at)",
        )
        .bind(policy_key)
        .bind(years)
        .bind(actor_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn search_patients(
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

    async fn list_dish_categories(&self) -> Result<Vec<DishCategoryDto>, ApiError> {
        let rows = sqlx::query_as::<_, (i64, String)>(
            "SELECT id, name FROM dish_categories ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(id, name)| DishCategoryDto { id, name })
            .collect())
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
        let result = sqlx::query(
            "INSERT INTO dishes
             (category_id, name, description, base_price_cents, photo_path, is_published, is_sold_out, created_by, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, FALSE, FALSE, ?, NOW(), NOW())",
        )
        .bind(category_id)
        .bind(name)
        .bind(description)
        .bind(base_price_cents)
        .bind(photo_path)
        .bind(actor_id)
        .execute(&self.pool)
        .await?;
        Ok(result.last_insert_id() as i64)
    }

    async fn set_dish_status(&self, dish_id: i64, is_published: bool, is_sold_out: bool) -> Result<(), ApiError> {
        sqlx::query("UPDATE dishes SET is_published = ?, is_sold_out = ?, updated_at = NOW() WHERE id = ?")
            .bind(is_published)
            .bind(is_sold_out)
            .bind(dish_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn add_dish_option(&self, dish_id: i64, option_group: &str, option_value: &str, delta_price_cents: i32) -> Result<(), ApiError> {
        sqlx::query(
            "INSERT INTO dish_options (dish_id, option_group, option_value, delta_price_cents) VALUES (?, ?, ?, ?)",
        )
        .bind(dish_id)
        .bind(option_group)
        .bind(option_value)
        .bind(delta_price_cents)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn add_sales_window(&self, dish_id: i64, slot_name: &str, start_hhmm: &str, end_hhmm: &str) -> Result<(), ApiError> {
        sqlx::query(
            "INSERT INTO dish_sales_windows (dish_id, slot_name, start_hhmm, end_hhmm) VALUES (?, ?, ?, ?)",
        )
        .bind(dish_id)
        .bind(slot_name)
        .bind(start_hhmm)
        .bind(end_hhmm)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn list_dishes(&self) -> Result<Vec<DishDto>, ApiError> {
        let rows = sqlx::query_as::<_, (i64, String, String, String, i32, String, bool, bool)>(
            "SELECT d.id, c.name, d.name, d.description, d.base_price_cents, d.photo_path, d.is_published, d.is_sold_out
             FROM dishes d JOIN dish_categories c ON c.id = d.category_id ORDER BY d.id DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| DishDto {
                id: r.0,
                category: r.1,
                name: r.2,
                description: r.3,
                base_price_cents: r.4,
                photo_path: r.5,
                is_published: r.6,
                is_sold_out: r.7,
            })
            .collect())
    }

    async fn upsert_ranking_rule(&self, rule_key: &str, weight: f64, enabled: bool, actor_id: i64) -> Result<(), ApiError> {
        sqlx::query(
            "INSERT INTO ranking_rules (rule_key, weight, enabled, updated_by, updated_at)
             VALUES (?, ?, ?, ?, NOW())
             ON DUPLICATE KEY UPDATE weight = VALUES(weight), enabled = VALUES(enabled), updated_by = VALUES(updated_by), updated_at = VALUES(updated_at)",
        )
        .bind(rule_key)
        .bind(weight)
        .bind(enabled)
        .bind(actor_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn list_ranking_rules(&self) -> Result<Vec<RankingRuleDto>, ApiError> {
        let rows = sqlx::query_as::<_, (String, f64, bool)>(
            "SELECT rule_key, weight, enabled FROM ranking_rules ORDER BY rule_key",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(rule_key, weight, enabled)| RankingRuleDto {
                rule_key,
                weight,
                enabled,
            })
            .collect())
    }

    async fn recommendations(&self) -> Result<Vec<RecommendationDto>, ApiError> {
        let ctr_weight = sqlx::query_scalar::<_, Option<f64>>(
            "SELECT weight FROM ranking_rules WHERE rule_key = 'ctr_weight' AND enabled = TRUE",
        )
        .fetch_one(&self.pool)
        .await?
        .unwrap_or(0.5);
        let conv_weight = sqlx::query_scalar::<_, Option<f64>>(
            "SELECT weight FROM ranking_rules WHERE rule_key = 'conversion_weight' AND enabled = TRUE",
        )
        .fetch_one(&self.pool)
        .await?
        .unwrap_or(0.5);

        let rows = sqlx::query_as::<_, (i64, i64, i64)>(
            "SELECT d.id,
                    SUM(CASE WHEN te.event_name = 'recommendation_click' THEN 1 ELSE 0 END) AS clicks,
                    SUM(CASE WHEN te.event_name = 'order_created' THEN 1 ELSE 0 END) AS conversions
             FROM dishes d
             LEFT JOIN telemetry_events te ON te.payload_json LIKE CONCAT('%\"dish_id\":', d.id, '%')
             WHERE d.is_published = TRUE AND d.is_sold_out = FALSE
             GROUP BY d.id
             ORDER BY d.id",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(dish_id, clicks, conversions)| RecommendationDto {
                dish_id,
                score: (clicks as f64 * ctr_weight) + (conversions as f64 * conv_weight),
            })
            .collect())
    }

    async fn close_inactive_campaigns(&self) -> Result<(), ApiError> {
        sqlx::query(
            "UPDATE group_campaigns gc
             JOIN (
                 SELECT gc_inner.id AS campaign_id,
                        COUNT(o.id) AS qualifying_orders
                 FROM group_campaigns gc_inner
                 LEFT JOIN campaign_members cm ON cm.campaign_id = gc_inner.id
                 LEFT JOIN dishes d ON d.id = gc_inner.dish_id
                 LEFT JOIN dining_menus dm ON dm.item_name = d.name
                 LEFT JOIN dining_orders o
                    ON o.menu_id = dm.id
                   AND o.created_by = cm.user_id
                   AND o.created_at >= gc_inner.created_at
                   AND o.created_at <= gc_inner.success_deadline_at
                   AND o.status IN ('Created', 'Billed')
                 GROUP BY gc_inner.id
             ) qc ON qc.campaign_id = gc.id
             SET gc.status = 'Successful', gc.closed_at = COALESCE(gc.closed_at, NOW())
             WHERE gc.status = 'Open' AND qc.qualifying_orders >= gc.success_threshold",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "UPDATE group_campaigns
             SET status = 'Closed', closed_at = NOW()
             WHERE status = 'Open'
               AND (TIMESTAMPDIFF(MINUTE, last_activity_at, NOW()) >= 30 OR NOW() > success_deadline_at)",
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn create_campaign(
        &self,
        title: &str,
        dish_id: i64,
        success_threshold: i32,
        success_deadline_at: &str,
        actor_id: i64,
    ) -> Result<i64, ApiError> {
        let result = sqlx::query(
            "INSERT INTO group_campaigns
             (title, dish_id, success_threshold, success_deadline_at, status, created_by, last_activity_at, created_at, closed_at)
             VALUES (?, ?, ?, ?, 'Open', ?, NOW(), NOW(), NULL)",
        )
        .bind(title)
        .bind(dish_id)
        .bind(success_threshold)
        .bind(success_deadline_at)
        .bind(actor_id)
        .execute(&self.pool)
        .await?;
        Ok(result.last_insert_id() as i64)
    }

    async fn join_campaign(&self, campaign_id: i64, user_id: i64) -> Result<(), ApiError> {
        sqlx::query(
            "INSERT IGNORE INTO campaign_members (campaign_id, user_id, joined_at) VALUES (?, ?, NOW())",
        )
        .bind(campaign_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        sqlx::query("UPDATE group_campaigns SET last_activity_at = NOW() WHERE id = ?")
            .bind(campaign_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn list_campaigns(&self) -> Result<Vec<CampaignDto>, ApiError> {
        let rows = sqlx::query_as::<_, (i64, String, i64, i32, String, String, i32, i32, String)>(
            "SELECT gc.id, gc.title, gc.dish_id, gc.success_threshold,
                    DATE_FORMAT(gc.success_deadline_at, '%Y-%m-%d %H:%i:%s'), gc.status,
                    COALESCE(cm.members, 0), COALESCE(qo.qualifying_orders, 0),
                    DATE_FORMAT(gc.last_activity_at, '%Y-%m-%d %H:%i:%s')
              FROM group_campaigns gc
              LEFT JOIN (SELECT campaign_id, COUNT(1) AS members FROM campaign_members GROUP BY campaign_id) cm
                 ON cm.campaign_id = gc.id
              LEFT JOIN (
                  SELECT gc_inner.id AS campaign_id, COUNT(o.id) AS qualifying_orders
                  FROM group_campaigns gc_inner
                  LEFT JOIN campaign_members cmm ON cmm.campaign_id = gc_inner.id
                  LEFT JOIN dishes d ON d.id = gc_inner.dish_id
                  LEFT JOIN dining_menus dm ON dm.item_name = d.name
                  LEFT JOIN dining_orders o
                     ON o.menu_id = dm.id
                    AND o.created_by = cmm.user_id
                    AND o.created_at >= gc_inner.created_at
                    AND o.created_at <= gc_inner.success_deadline_at
                    AND o.status IN ('Created', 'Billed')
                  GROUP BY gc_inner.id
              ) qo ON qo.campaign_id = gc.id
              ORDER BY gc.id DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| CampaignDto {
                id: r.0,
                title: r.1,
                dish_id: r.2,
                success_threshold: r.3,
                success_deadline_at: r.4,
                status: r.5,
                participants: r.6,
                qualifying_orders: r.7,
                last_activity_at: r.8,
            })
            .collect())
    }

    async fn add_ticket_split(&self, order_id: i64, split_by: &str, split_value: &str, quantity: i32) -> Result<(), ApiError> {
        sqlx::query(
            "INSERT INTO order_tickets (order_id, split_by, split_value, quantity) VALUES (?, ?, ?, ?)",
        )
        .bind(order_id)
        .bind(split_by)
        .bind(split_value)
        .bind(quantity)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn list_ticket_splits(&self, order_id: i64) -> Result<Vec<TicketSplitDto>, ApiError> {
        let rows = sqlx::query_as::<_, (i64, String, String, i32)>(
            "SELECT id, split_by, split_value, quantity
             FROM order_tickets WHERE order_id = ? ORDER BY id DESC",
        )
        .bind(order_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| TicketSplitDto {
                id: r.0,
                split_by: r.1,
                split_value: r.2,
                quantity: r.3,
            })
            .collect())
    }

    async fn add_order_note(&self, order_id: i64, note: &str, staff_user_id: i64) -> Result<(), ApiError> {
        sqlx::query(
            "INSERT INTO order_operation_notes (order_id, note, staff_user_id, created_at) VALUES (?, ?, ?, NOW())",
        )
        .bind(order_id)
        .bind(note)
        .bind(staff_user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn list_order_notes(&self, order_id: i64) -> Result<Vec<OrderNoteDto>, ApiError> {
        let rows = sqlx::query_as::<_, (i64, String, String, String)>(
            "SELECT n.id, n.note, u.username, DATE_FORMAT(n.created_at, '%Y-%m-%d %H:%i:%s')
             FROM order_operation_notes n
             JOIN users u ON u.id = n.staff_user_id
             WHERE n.order_id = ? ORDER BY n.id DESC",
        )
        .bind(order_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| OrderNoteDto {
                id: r.0,
                note: r.1,
                staff_username: r.2,
                created_at: r.3,
            })
            .collect())
    }

    async fn create_experiment(&self, experiment_key: &str) -> Result<i64, ApiError> {
        let result = sqlx::query(
            "INSERT INTO experiments (experiment_key, is_active) VALUES (?, TRUE)
             ON DUPLICATE KEY UPDATE is_active = TRUE",
        )
        .bind(experiment_key)
        .execute(&self.pool)
        .await?;
        let id = if result.last_insert_id() == 0 {
            sqlx::query_scalar::<_, i64>("SELECT id FROM experiments WHERE experiment_key = ?")
                .bind(experiment_key)
                .fetch_one(&self.pool)
                .await?
        } else {
            result.last_insert_id() as i64
        };
        Ok(id)
    }

    async fn add_experiment_variant(
        &self,
        experiment_id: i64,
        variant_key: &str,
        allocation_weight: f64,
        feature_version: &str,
    ) -> Result<(), ApiError> {
        sqlx::query(
            "INSERT INTO experiment_variants (experiment_id, variant_key, allocation_weight, feature_version, active)
             VALUES (?, ?, ?, ?, TRUE)
             ON DUPLICATE KEY UPDATE allocation_weight = VALUES(allocation_weight), feature_version = VALUES(feature_version), active = TRUE",
        )
        .bind(experiment_id)
        .bind(variant_key)
        .bind(allocation_weight)
        .bind(feature_version)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn assign_experiment_variant(
        &self,
        experiment_id: i64,
        user_id: i64,
        mode: &str,
    ) -> Result<Option<String>, ApiError> {
        let existing = sqlx::query_as::<_, (String,)>(
            "SELECT v.variant_key
             FROM experiment_assignments a
             JOIN experiment_variants v ON v.id = a.variant_id
             WHERE a.experiment_id = ? AND a.user_id = ?",
        )
        .bind(experiment_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        if let Some((variant,)) = existing {
            return Ok(Some(variant));
        }

        let variant_row = if mode == "bandit" {
            sqlx::query_as::<_, (i64, String)>(
                "SELECT v.id, v.variant_key
                 FROM experiment_variants v
                 LEFT JOIN (
                    SELECT variant_id,
                           SUM(CASE WHEN te.event_name = 'recommendation_click' THEN 1 ELSE 0 END) AS clicks,
                           SUM(CASE WHEN te.event_name = 'order_created' THEN 1 ELSE 0 END) AS conversions
                    FROM experiment_assignments a
                    LEFT JOIN telemetry_events te ON te.user_id = a.user_id
                    GROUP BY variant_id
                 ) s ON s.variant_id = v.id
                 WHERE v.experiment_id = ? AND v.active = TRUE
                 ORDER BY (COALESCE(s.conversions,0) * 1.0 / NULLIF(COALESCE(s.clicks,0),0)) DESC,
                          v.allocation_weight DESC
                 LIMIT 1",
            )
            .bind(experiment_id)
            .fetch_optional(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, (i64, String)>(
                "SELECT id, variant_key FROM experiment_variants
                 WHERE experiment_id = ? AND active = TRUE
                 ORDER BY allocation_weight DESC, id ASC LIMIT 1",
            )
            .bind(experiment_id)
            .fetch_optional(&self.pool)
            .await?
        };

        if let Some((variant_id, variant_key)) = variant_row {
            sqlx::query(
                "INSERT INTO experiment_assignments (experiment_id, user_id, variant_id, assigned_at)
                 VALUES (?, ?, ?, NOW())",
            )
            .bind(experiment_id)
            .bind(user_id)
            .bind(variant_id)
            .execute(&self.pool)
            .await?;
            Ok(Some(variant_key))
        } else {
            Ok(None)
        }
    }

    async fn record_experiment_backtrack(
        &self,
        experiment_id: i64,
        from_version: &str,
        to_version: &str,
        reason: &str,
        actor_id: i64,
    ) -> Result<(), ApiError> {
        sqlx::query(
            "INSERT INTO experiment_backtracks (experiment_id, from_version, to_version, reason, actor_id, created_at)
             VALUES (?, ?, ?, ?, ?, NOW())",
        )
        .bind(experiment_id)
        .bind(from_version)
        .bind(to_version)
        .bind(reason)
        .bind(actor_id)
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "UPDATE experiment_variants SET active = CASE WHEN feature_version = ? THEN TRUE ELSE FALSE END WHERE experiment_id = ?",
        )
        .bind(to_version)
        .bind(experiment_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn funnel_metrics(&self) -> Result<Vec<FunnelMetricsDto>, ApiError> {
        let login_users = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(DISTINCT user_id) FROM sessions WHERE revoked_at IS NULL",
        )
        .fetch_one(&self.pool)
        .await?;
        let patient_edit_users = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(DISTINCT actor_id) FROM audit_logs WHERE action_type IN ('patient.edit','clinical.edit')",
        )
        .fetch_one(&self.pool)
        .await?;
        let order_users = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(DISTINCT created_by) FROM dining_orders",
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(vec![
            FunnelMetricsDto {
                step: "login".to_string(),
                users: login_users,
            },
            FunnelMetricsDto {
                step: "workflow_action".to_string(),
                users: patient_edit_users,
            },
            FunnelMetricsDto {
                step: "dining_order".to_string(),
                users: order_users,
            },
        ])
    }

    async fn retention_metrics(&self) -> Result<Vec<RetentionMetricsDto>, ApiError> {
        let day1 = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(DISTINCT user_id)
             FROM telemetry_events
             WHERE created_at >= DATE_SUB(NOW(), INTERVAL 1 DAY)",
        )
        .fetch_one(&self.pool)
        .await?;
        let day7 = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(DISTINCT user_id)
             FROM telemetry_events
             WHERE created_at >= DATE_SUB(NOW(), INTERVAL 7 DAY)",
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(vec![
            RetentionMetricsDto {
                cohort: "1d".to_string(),
                retained_users: day1,
            },
            RetentionMetricsDto {
                cohort: "7d".to_string(),
                retained_users: day7,
            },
        ])
    }

    async fn recommendation_kpi(&self) -> Result<RecommendationKpiDto, ApiError> {
        let clicks = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(1) FROM telemetry_events WHERE event_name = 'recommendation_click'",
        )
        .fetch_one(&self.pool)
        .await?;
        let impressions = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(1) FROM telemetry_events WHERE event_name = 'recommendation_impression'",
        )
        .fetch_one(&self.pool)
        .await?;
        let conversions = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(1) FROM telemetry_events WHERE event_name = 'order_created'",
        )
        .fetch_one(&self.pool)
        .await?;
        let ctr = if impressions == 0 {
            0.0
        } else {
            clicks as f64 / impressions as f64
        };
        let conversion = if clicks == 0 {
            0.0
        } else {
            conversions as f64 / clicks as f64
        };
        Ok(RecommendationKpiDto { ctr, conversion })
    }

    async fn create_ingestion_task(
        &self,
        actor_id: i64,
        req: IngestionTaskCreateRequest,
    ) -> Result<i64, ApiError> {
        let schedule_cron = req.schedule_cron.trim().to_string();
        let next_run = Self::next_run_iso(&schedule_cron)?;
        let result = sqlx::query(
            "INSERT INTO ingestion_tasks
             (task_name, status, active_version, schedule_cron, max_depth, pagination_strategy, incremental_field, next_run_at, last_run_at, created_by, created_at, updated_at)
             VALUES (?, 'active', 1, ?, ?, ?, ?, STR_TO_DATE(?, '%Y-%m-%dT%H:%i:%sZ'), NULL, ?, NOW(), NOW())",
        )
        .bind(req.task_name.trim())
        .bind(&schedule_cron)
        .bind(req.max_depth)
        .bind(req.pagination_strategy.trim())
        .bind(req.incremental_field.as_deref())
        .bind(next_run)
        .bind(actor_id)
        .execute(&self.pool)
        .await?;
        let task_id = result.last_insert_id() as i64;

        sqlx::query(
            "INSERT INTO ingestion_task_versions
             (task_id, version_number, seed_urls_json, extraction_rules_json, rollback_of_version, created_by, created_at)
             VALUES (?, 1, ?, ?, NULL, ?, NOW())",
        )
        .bind(task_id)
        .bind(serde_json::to_string(&req.seed_urls).map_err(|_| ApiError::Internal)?)
        .bind(req.extraction_rules_json)
        .bind(actor_id)
        .execute(&self.pool)
        .await?;
        Ok(task_id)
    }

    async fn update_ingestion_task(
        &self,
        task_id: i64,
        actor_id: i64,
        actor_role: &str,
        req: IngestionTaskUpdateRequest,
    ) -> Result<i32, ApiError> {
        let schedule_cron = req.schedule_cron.trim().to_string();
        let next_run = Self::next_run_iso(&schedule_cron)?;
        let current_version = if self.has_global_ingestion_access(actor_role).await? {
            sqlx::query_scalar::<_, Option<i32>>(
                "SELECT COALESCE(MAX(version_number), 0) FROM ingestion_task_versions WHERE task_id = ?",
            )
            .bind(task_id)
            .fetch_one(&self.pool)
            .await?
            .ok_or(ApiError::NotFound)?
        } else {
            sqlx::query_scalar::<_, Option<i32>>(
                "SELECT COALESCE(MAX(v.version_number), 0)
                 FROM ingestion_task_versions v
                 JOIN ingestion_tasks t ON t.id = v.task_id
                 WHERE v.task_id = ? AND t.created_by = ?",
            )
            .bind(task_id)
            .bind(actor_id)
            .fetch_one(&self.pool)
            .await?
            .ok_or(ApiError::NotFound)?
        };
        let next_version = current_version + 1;

        sqlx::query(
            "INSERT INTO ingestion_task_versions
             (task_id, version_number, seed_urls_json, extraction_rules_json, rollback_of_version, created_by, created_at)
             VALUES (?, ?, ?, ?, NULL, ?, NOW())",
        )
        .bind(task_id)
        .bind(next_version)
        .bind(serde_json::to_string(&req.seed_urls).map_err(|_| ApiError::Internal)?)
        .bind(req.extraction_rules_json)
        .bind(actor_id)
        .execute(&self.pool)
        .await?;

        let updated = if self.has_global_ingestion_access(actor_role).await? {
            sqlx::query(
                "UPDATE ingestion_tasks
                 SET active_version = ?, schedule_cron = ?, max_depth = ?, pagination_strategy = ?, incremental_field = ?, next_run_at = STR_TO_DATE(?, '%Y-%m-%dT%H:%i:%sZ'), updated_at = NOW()
                 WHERE id = ?",
            )
            .bind(next_version)
            .bind(&schedule_cron)
            .bind(req.max_depth)
            .bind(req.pagination_strategy.trim())
            .bind(req.incremental_field.as_deref())
            .bind(&next_run)
            .bind(task_id)
            .execute(&self.pool)
            .await?
            .rows_affected()
        } else {
            sqlx::query(
                "UPDATE ingestion_tasks
                 SET active_version = ?, schedule_cron = ?, max_depth = ?, pagination_strategy = ?, incremental_field = ?, next_run_at = STR_TO_DATE(?, '%Y-%m-%dT%H:%i:%sZ'), updated_at = NOW()
                 WHERE id = ? AND created_by = ?",
            )
            .bind(next_version)
            .bind(&schedule_cron)
            .bind(req.max_depth)
            .bind(req.pagination_strategy.trim())
            .bind(req.incremental_field.as_deref())
            .bind(&next_run)
            .bind(task_id)
            .bind(actor_id)
            .execute(&self.pool)
            .await?
            .rows_affected()
        };

        if updated == 0 {
            return Err(ApiError::NotFound);
        }

        Ok(next_version)
    }

    async fn rollback_ingestion_task(
        &self,
        task_id: i64,
        actor_id: i64,
        actor_role: &str,
        req: IngestionTaskRollbackRequest,
    ) -> Result<i32, ApiError> {
        let target = if self.has_global_ingestion_access(actor_role).await? {
            sqlx::query_as::<_, (String, String, String)>(
                "SELECT v.seed_urls_json, v.extraction_rules_json, t.schedule_cron
                 FROM ingestion_task_versions v
                 JOIN ingestion_tasks t ON t.id = v.task_id
                 WHERE v.task_id = ? AND v.version_number = ?",
            )
            .bind(task_id)
            .bind(req.target_version)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(ApiError::NotFound)?
        } else {
            sqlx::query_as::<_, (String, String, String)>(
                "SELECT v.seed_urls_json, v.extraction_rules_json, t.schedule_cron
                 FROM ingestion_task_versions v
                 JOIN ingestion_tasks t ON t.id = v.task_id
                 WHERE v.task_id = ? AND v.version_number = ? AND t.created_by = ?",
            )
            .bind(task_id)
            .bind(req.target_version)
            .bind(actor_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(ApiError::NotFound)?
        };
        let next_run = Self::next_run_iso(&target.2)?;

        let current_version = if self.has_global_ingestion_access(actor_role).await? {
            sqlx::query_scalar::<_, Option<i32>>(
                "SELECT COALESCE(MAX(version_number), 0) FROM ingestion_task_versions WHERE task_id = ?",
            )
            .bind(task_id)
            .fetch_one(&self.pool)
            .await?
            .ok_or(ApiError::NotFound)?
        } else {
            sqlx::query_scalar::<_, Option<i32>>(
                "SELECT COALESCE(MAX(v.version_number), 0)
                 FROM ingestion_task_versions v
                 JOIN ingestion_tasks t ON t.id = v.task_id
                 WHERE v.task_id = ? AND t.created_by = ?",
            )
            .bind(task_id)
            .bind(actor_id)
            .fetch_one(&self.pool)
            .await?
            .ok_or(ApiError::NotFound)?
        };
        let next_version = current_version + 1;

        sqlx::query(
            "INSERT INTO ingestion_task_versions
             (task_id, version_number, seed_urls_json, extraction_rules_json, rollback_of_version, created_by, created_at)
             VALUES (?, ?, ?, ?, ?, ?, NOW())",
        )
        .bind(task_id)
        .bind(next_version)
        .bind(target.0)
        .bind(target.1)
        .bind(req.target_version)
        .bind(actor_id)
        .execute(&self.pool)
        .await?;

        let updated = if self.has_global_ingestion_access(actor_role).await? {
            sqlx::query(
                "UPDATE ingestion_tasks SET active_version = ?, next_run_at = STR_TO_DATE(?, '%Y-%m-%dT%H:%i:%sZ'), updated_at = NOW() WHERE id = ?",
            )
            .bind(next_version)
            .bind(&next_run)
            .bind(task_id)
            .execute(&self.pool)
            .await?
            .rows_affected()
        } else {
            sqlx::query(
                "UPDATE ingestion_tasks SET active_version = ?, next_run_at = STR_TO_DATE(?, '%Y-%m-%dT%H:%i:%sZ'), updated_at = NOW() WHERE id = ? AND created_by = ?",
            )
            .bind(next_version)
            .bind(&next_run)
            .bind(task_id)
            .bind(actor_id)
            .execute(&self.pool)
            .await?
            .rows_affected()
        };
        if updated == 0 {
            return Err(ApiError::NotFound);
        }
        Ok(next_version)
    }

    async fn run_ingestion_task(&self, task_id: i64, actor_id: i64, actor_role: &str) -> Result<(), ApiError> {
        let task = if self.has_global_ingestion_access(actor_role).await? {
            sqlx::query_as::<_, (i32, i32, String, Option<String>, String, String, Option<String>, String)>(
                "SELECT CAST(active_version AS SIGNED), max_depth, pagination_strategy, incremental_field,
                        v.seed_urls_json, v.extraction_rules_json, t.last_incremental_value, t.schedule_cron
                 FROM ingestion_tasks t
                 JOIN ingestion_task_versions v ON v.task_id = t.id AND v.version_number = t.active_version
                 WHERE t.id = ?",
            )
            .bind(task_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(ApiError::NotFound)?
        } else {
            sqlx::query_as::<_, (i32, i32, String, Option<String>, String, String, Option<String>, String)>(
                "SELECT CAST(active_version AS SIGNED), max_depth, pagination_strategy, incremental_field,
                        v.seed_urls_json, v.extraction_rules_json, t.last_incremental_value, t.schedule_cron
                 FROM ingestion_tasks t
                 JOIN ingestion_task_versions v ON v.task_id = t.id AND v.version_number = t.active_version
                 WHERE t.id = ? AND t.created_by = ?",
            )
            .bind(task_id)
            .bind(actor_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(ApiError::NotFound)?
        };

        let active_version = task.0;
        let max_depth = task.1.max(0) as usize;
        let pagination_strategy = task.2;
        let incremental_field = task.3;
        let seed_urls_json = task.4;
        let extraction_rules_json = task.5;
        let previous_incremental = task.6;
        let schedule_cron = task.7;

        let run_result = sqlx::query(
            "INSERT INTO ingestion_task_runs (task_id, task_version, status, started_at, finished_at, records_extracted, diagnostics_json)
             VALUES (?, ?, 'running', NOW(), NULL, 0, '{}')",
        )
        .bind(task_id)
        .bind(active_version)
        .execute(&self.pool)
        .await?;
        let run_id = run_result.last_insert_id() as i64;

        let execution_result: Result<(String, i32, serde_json::Value, Option<String>), ApiError> = async {
            let seed_urls: Vec<String> = serde_json::from_str(&seed_urls_json)
                .map_err(|_| ApiError::bad_request("Invalid seed URL config"))?;
            let (mode, fields, pagination_selector) =
                Self::parse_extraction_config(&extraction_rules_json)?;
            let mut queue: VecDeque<(String, usize)> = seed_urls.into_iter().map(|u| (u, 0usize)).collect();
            let mut seen = HashSet::new();
            let mut pages_visited = 0i32;
            let mut records_persisted = 0i32;
            let mut skipped_incremental = 0i32;
            let mut fetch_errors = 0i32;
            let mut fetch_error_urls: Vec<String> = Vec::new();
            let mut max_incremental = previous_incremental.clone();

            while let Some((url, depth)) = if pagination_strategy.eq_ignore_ascii_case("depth-first") {
                queue.pop_back()
            } else {
                queue.pop_front()
            } {
                if depth > max_depth {
                    continue;
                }
                if !seen.insert(url.clone()) {
                    continue;
                }

                let content = match Self::fetch_source(&url).await {
                    Ok(v) => v,
                    Err(_) => {
                        fetch_errors += 1;
                        fetch_error_urls.push(url.clone());
                        continue;
                    }
                };
                pages_visited += 1;

                let records = Self::extract_records(&mode, &fields, &content, &url)?;
                for record in records {
                    let incremental_value = incremental_field
                        .as_deref()
                        .and_then(|key| record.get(key))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .or_else(|| {
                            record
                                .get("value")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                        });

                    if let (Some(previous), Some(current)) =
                        (previous_incremental.as_deref(), incremental_value.as_deref())
                    {
                        if current <= previous {
                            skipped_incremental += 1;
                            continue;
                        }
                    }

                    if let Some(current) = incremental_value.as_deref() {
                        if max_incremental
                            .as_deref()
                            .map(|v| current > v)
                            .unwrap_or(true)
                        {
                            max_incremental = Some(current.to_string());
                        }
                    }

                    let serialized = serde_json::to_string(&record).map_err(|_| ApiError::Internal)?;
                    let hash = hex::encode(Sha256::digest(serialized.as_bytes()));
                    let inserted = sqlx::query(
                        "INSERT IGNORE INTO ingestion_task_records (task_id, run_id, source_url, record_json, content_hash, incremental_value, extracted_at)
                         VALUES (?, ?, ?, ?, ?, ?, NOW())",
                    )
                    .bind(task_id)
                    .bind(run_id)
                    .bind(&url)
                    .bind(serialized)
                    .bind(hash)
                    .bind(incremental_value)
                    .execute(&self.pool)
                    .await?
                    .rows_affected();
                    if inserted > 0 {
                        records_persisted += 1;
                    }
                }

                if depth < max_depth {
                    for next in Self::discover_links(&url, &content, pagination_selector.as_deref()) {
                        if !seen.contains(&next) {
                            queue.push_back((next, depth + 1));
                        }
                    }
                }
            }

            fetch_error_urls.sort();
            fetch_error_urls.dedup();
            let diagnostics = serde_json::json!({
                "mode": mode,
                "pages_visited": pages_visited,
                "records_persisted": records_persisted,
                "skipped_incremental": skipped_incremental,
                "fetch_errors": fetch_errors,
                "fetch_error_urls": fetch_error_urls,
                "max_depth": max_depth,
                "strategy": pagination_strategy,
                "deterministic": true
            });
            let status = if pages_visited == 0 && fetch_errors > 0 {
                "failed".to_string()
            } else {
                "success".to_string()
            };
            Ok((status, records_persisted, diagnostics, max_incremental))
        }
        .await;

        match execution_result {
            Ok((status, records_persisted, diagnostics, max_incremental)) => {
                let next_run = Self::next_run_iso(&schedule_cron)?;
                if status == "failed" {
                    tracing::warn!(
                        category = "ingestion",
                        event = "ingestion.execution",
                        outcome = "failed",
                        task_id = task_id,
                        run_id = run_id,
                        records_extracted = records_persisted,
                        deterministic = diagnostics.get("deterministic").and_then(|v| v.as_bool()).unwrap_or(false),
                        "ingestion_task_failed"
                    );
                }
                sqlx::query(
                    "UPDATE ingestion_task_runs
                     SET status = ?, finished_at = NOW(), records_extracted = ?, diagnostics_json = ?
                     WHERE id = ?",
                )
                .bind(&status)
                .bind(records_persisted)
                .bind(serde_json::to_string(&diagnostics).map_err(|_| ApiError::Internal)?)
                .bind(run_id)
                .execute(&self.pool)
                .await?;

                sqlx::query(
                    "UPDATE ingestion_tasks
                     SET last_run_at = NOW(),
                         next_run_at = COALESCE(STR_TO_DATE(?, '%Y-%m-%dT%H:%i:%sZ'), next_run_at),
                         last_incremental_value = ?,
                         updated_at = NOW()
                     WHERE id = ?",
                )
                .bind(Some(next_run.as_str()))
                .bind(max_incremental)
                .bind(task_id)
                .execute(&self.pool)
                .await?;

                Ok(())
            }
            Err(err) => {
                tracing::error!(
                    category = "ingestion",
                    event = "ingestion.execution",
                    outcome = "failed",
                    task_id = task_id,
                    run_id = run_id,
                    error_code = "execution_error",
                    "ingestion_execution_error"
                );
                let diagnostics = serde_json::json!({
                    "deterministic": true,
                    "failure": err.to_string()
                });
                let _ = sqlx::query(
                    "UPDATE ingestion_task_runs
                     SET status = 'failed', finished_at = NOW(), records_extracted = 0, diagnostics_json = ?
                     WHERE id = ?",
                )
                .bind(serde_json::to_string(&diagnostics).unwrap_or_else(|_| "{}".to_string()))
                .bind(run_id)
                .execute(&self.pool)
                .await;
                let _ = sqlx::query(
                    "UPDATE ingestion_tasks
                     SET last_run_at = NOW(),
                         next_run_at = COALESCE(STR_TO_DATE(?, '%Y-%m-%dT%H:%i:%sZ'), next_run_at),
                         updated_at = NOW()
                     WHERE id = ?",
                )
                .bind(Self::next_run_iso(&schedule_cron).ok())
                .bind(task_id)
                .execute(&self.pool)
                .await;
                Err(err)
            }
        }
    }

    async fn list_ingestion_tasks(&self, actor_id: i64, actor_role: &str) -> Result<Vec<IngestionTaskDto>, ApiError> {
        let rows = if self.has_global_ingestion_access(actor_role).await? {
            sqlx::query_as::<_, (i64, String, String, i32, String, i32, String, Option<String>, Option<String>, Option<String>)>(
                "SELECT id, task_name, status, COALESCE(active_version,0), schedule_cron, max_depth, pagination_strategy, incremental_field,
                        DATE_FORMAT(next_run_at, '%Y-%m-%d %H:%i:%s'), DATE_FORMAT(last_run_at, '%Y-%m-%d %H:%i:%s')
                 FROM ingestion_tasks ORDER BY id DESC",
            )
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, (i64, String, String, i32, String, i32, String, Option<String>, Option<String>, Option<String>)>(
                "SELECT id, task_name, status, COALESCE(active_version,0), schedule_cron, max_depth, pagination_strategy, incremental_field,
                        DATE_FORMAT(next_run_at, '%Y-%m-%d %H:%i:%s'), DATE_FORMAT(last_run_at, '%Y-%m-%d %H:%i:%s')
                 FROM ingestion_tasks
                 WHERE created_by = ?
                 ORDER BY id DESC",
            )
            .bind(actor_id)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows
            .into_iter()
            .map(|r| IngestionTaskDto {
                id: r.0,
                task_name: r.1,
                status: r.2,
                active_version: r.3,
                schedule_cron: r.4,
                max_depth: r.5,
                pagination_strategy: r.6,
                incremental_field: r.7,
                next_run_at: r.8,
                last_run_at: r.9,
            })
            .collect())
    }

    async fn ingestion_task_versions(
        &self,
        task_id: i64,
        actor_id: i64,
        actor_role: &str,
    ) -> Result<Vec<IngestionTaskVersionDto>, ApiError> {
        let rows = if self.has_global_ingestion_access(actor_role).await? {
            sqlx::query_as::<_, (i64, i32, String, String, Option<i32>, String)>(
                "SELECT task_id, version_number, seed_urls_json, extraction_rules_json, rollback_of_version,
                        DATE_FORMAT(created_at, '%Y-%m-%d %H:%i:%s')
                 FROM ingestion_task_versions
                 WHERE task_id = ? ORDER BY version_number DESC",
            )
            .bind(task_id)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, (i64, i32, String, String, Option<i32>, String)>(
                "SELECT v.task_id, v.version_number, v.seed_urls_json, v.extraction_rules_json, v.rollback_of_version,
                        DATE_FORMAT(v.created_at, '%Y-%m-%d %H:%i:%s')
                 FROM ingestion_task_versions v
                 JOIN ingestion_tasks t ON t.id = v.task_id
                 WHERE v.task_id = ? AND t.created_by = ?
                 ORDER BY v.version_number DESC",
            )
            .bind(task_id)
            .bind(actor_id)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows
            .into_iter()
            .map(|r| IngestionTaskVersionDto {
                task_id: r.0,
                version_number: r.1,
                seed_urls_json: r.2,
                extraction_rules_json: r.3,
                rollback_of_version: r.4,
                created_at: r.5,
            })
            .collect())
    }

    async fn ingestion_task_runs(
        &self,
        task_id: i64,
        limit: i64,
        actor_id: i64,
        actor_role: &str,
    ) -> Result<Vec<IngestionTaskRunDto>, ApiError> {
        let safe_limit = limit.clamp(1, 100);
        let rows = if self.has_global_ingestion_access(actor_role).await? {
            sqlx::query_as::<_, (i64, i64, i32, String, String, Option<String>, i32, String)>(
                "SELECT id, task_id, task_version, status,
                        DATE_FORMAT(started_at, '%Y-%m-%d %H:%i:%s'),
                        DATE_FORMAT(finished_at, '%Y-%m-%d %H:%i:%s'),
                        records_extracted, diagnostics_json
                 FROM ingestion_task_runs
                 WHERE task_id = ?
                 ORDER BY id DESC
                 LIMIT ?",
            )
            .bind(task_id)
            .bind(safe_limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, (i64, i64, i32, String, String, Option<String>, i32, String)>(
                "SELECT r.id, r.task_id, r.task_version, r.status,
                        DATE_FORMAT(r.started_at, '%Y-%m-%d %H:%i:%s'),
                        DATE_FORMAT(r.finished_at, '%Y-%m-%d %H:%i:%s'),
                        r.records_extracted, r.diagnostics_json
                 FROM ingestion_task_runs r
                 JOIN ingestion_tasks t ON t.id = r.task_id
                 WHERE r.task_id = ? AND t.created_by = ?
                 ORDER BY r.id DESC
                 LIMIT ?",
            )
            .bind(task_id)
            .bind(actor_id)
            .bind(safe_limit)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows
            .into_iter()
            .map(|r| IngestionTaskRunDto {
                id: r.0,
                task_id: r.1,
                task_version: r.2,
                status: r.3,
                started_at: r.4,
                finished_at: r.5,
                records_extracted: r.6,
                diagnostics_json: r.7,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::MySqlAppRepository;

    #[test]
    fn cron_next_run_accepts_hourly_expression() {
        let next = MySqlAppRepository::next_run_iso("0 * * * *");
        assert!(next.is_ok());
    }

    #[test]
    fn cron_next_run_rejects_invalid_expression() {
        let next = MySqlAppRepository::next_run_iso("not-a-cron");
        assert!(next.is_err());
    }
}
