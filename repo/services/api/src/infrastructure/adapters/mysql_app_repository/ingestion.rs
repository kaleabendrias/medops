use std::collections::{HashSet, VecDeque};
use std::str::FromStr;

use chrono::{SecondsFormat, Utc};
use cron::Schedule;
use regex::Regex;
use scraper::{Html, Selector};
use sha2::{Digest, Sha256};
use contracts::{
    IngestionTaskCreateRequest, IngestionTaskDto, IngestionTaskRollbackRequest,
    IngestionTaskRunDto, IngestionTaskUpdateRequest, IngestionTaskVersionDto,
};

use crate::contracts::ApiError;
use super::MySqlAppRepository;

impl MySqlAppRepository {
    pub(super) fn next_run_iso(schedule_cron: &str) -> Result<String, ApiError> {
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

    pub(super) async fn create_ingestion_task_impl(
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

    pub(super) async fn update_ingestion_task_impl(
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

    pub(super) async fn rollback_ingestion_task_impl(
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

    pub(super) async fn run_ingestion_task_impl(&self, task_id: i64, actor_id: i64, actor_role: &str) -> Result<(), ApiError> {
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

    pub(super) async fn list_ingestion_tasks_impl(&self, actor_id: i64, actor_role: &str) -> Result<Vec<IngestionTaskDto>, ApiError> {
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

    pub(super) async fn ingestion_task_versions_impl(
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

    pub(super) async fn ingestion_task_runs_impl(
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
