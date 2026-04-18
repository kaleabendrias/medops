use contracts::{
    AuditLogDto, FunnelMetricsDto, GovernanceRecordDto, RecommendationKpiDto,
    RetentionMetricsDto, RetentionPolicyDto,
};
use sha2::{Digest, Sha256};

use crate::contracts::ApiError;
use super::MySqlAppRepository;

/// Pure tier-lineage validation extracted from the DB insert path so it can
/// be unit-tested without a real database connection.
pub(crate) fn validate_tier_lineage(tier: &str, lineage_source_id: Option<i64>) -> Result<(), ApiError> {
    match tier {
        "raw" => {
            if lineage_source_id.is_some() {
                return Err(ApiError::bad_request(
                    "raw governance records must not declare a lineage source",
                ));
            }
        }
        "cleaned" => {
            if lineage_source_id.is_none() {
                return Err(ApiError::bad_request(
                    "cleaned governance records require a lineage source from governance_raw",
                ));
            }
        }
        "analytics" => {
            if lineage_source_id.is_none() {
                return Err(ApiError::bad_request(
                    "analytics governance records require a lineage source from governance_cleaned",
                ));
            }
        }
        _ => {
            return Err(ApiError::bad_request("tier must be raw, cleaned, or analytics"));
        }
    }
    Ok(())
}

impl MySqlAppRepository {
    pub(super) async fn create_governance_record_impl(
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
        validate_tier_lineage(tier, lineage_source_id)?;
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

    pub(super) async fn list_governance_records_impl(&self) -> Result<Vec<GovernanceRecordDto>, ApiError> {
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

    pub(super) async fn tombstone_governance_record_impl(&self, record_id: i64, reason: &str) -> Result<(), ApiError> {
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

    pub(super) async fn create_telemetry_event_impl(&self, experiment_key: &str, user_id: i64, event_name: &str, payload_json: &str) -> Result<(), ApiError> {
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

    pub(super) async fn append_audit_impl(
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

    pub(super) async fn list_audits_impl(&self) -> Result<Vec<AuditLogDto>, ApiError> {
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

    pub(super) async fn list_retention_policies_impl(&self) -> Result<Vec<RetentionPolicyDto>, ApiError> {
        let rows = sqlx::query_as::<_, (String, i32)>("SELECT policy_key, years FROM retention_policies ORDER BY policy_key")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(|(policy_key, years)| RetentionPolicyDto { policy_key, years })
            .collect())
    }

    pub(super) async fn upsert_retention_policy_impl(&self, policy_key: &str, years: i32, actor_id: i64) -> Result<(), ApiError> {
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

    pub(super) async fn create_experiment_impl(&self, experiment_key: &str) -> Result<i64, ApiError> {
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

    pub(super) async fn add_experiment_variant_impl(
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

    pub(super) async fn assign_experiment_impl(
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

    pub(super) async fn backtrack_experiment_impl(
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

    pub(super) async fn funnel_metrics_impl(&self) -> Result<Vec<FunnelMetricsDto>, ApiError> {
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

    pub(super) async fn retention_metrics_impl(&self) -> Result<Vec<RetentionMetricsDto>, ApiError> {
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

    pub(super) async fn recommendation_kpi_impl(&self) -> Result<RecommendationKpiDto, ApiError> {
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
}

#[cfg(test)]
mod tests {
    use super::validate_tier_lineage;

    #[test]
    fn raw_tier_without_lineage_is_valid() {
        assert!(validate_tier_lineage("raw", None).is_ok());
    }

    #[test]
    fn raw_tier_with_lineage_source_is_invalid() {
        assert!(validate_tier_lineage("raw", Some(1)).is_err());
    }

    #[test]
    fn cleaned_tier_with_lineage_source_is_valid() {
        assert!(validate_tier_lineage("cleaned", Some(7)).is_ok());
    }

    #[test]
    fn cleaned_tier_without_lineage_source_is_invalid() {
        assert!(validate_tier_lineage("cleaned", None).is_err());
    }

    #[test]
    fn analytics_tier_with_lineage_source_is_valid() {
        assert!(validate_tier_lineage("analytics", Some(42)).is_ok());
    }

    #[test]
    fn analytics_tier_without_lineage_source_is_invalid() {
        assert!(validate_tier_lineage("analytics", None).is_err());
    }

    #[test]
    fn unknown_tier_is_invalid_regardless_of_lineage() {
        assert!(validate_tier_lineage("bronze", None).is_err());
        assert!(validate_tier_lineage("bronze", Some(1)).is_err());
        assert!(validate_tier_lineage("", None).is_err());
    }

    #[test]
    fn raw_tier_error_mentions_lineage_source() {
        let err = validate_tier_lineage("raw", Some(1)).unwrap_err();
        let msg = format!("{:?}", err);
        assert!(msg.contains("lineage source") || msg.contains("lineage"));
    }

    #[test]
    fn cleaned_tier_error_mentions_governance_raw() {
        let err = validate_tier_lineage("cleaned", None).unwrap_err();
        let msg = format!("{:?}", err);
        assert!(msg.contains("governance_raw") || msg.contains("lineage source"));
    }

    #[test]
    fn tier_casing_must_be_lowercase() {
        assert!(validate_tier_lineage("Raw", None).is_err());
        assert!(validate_tier_lineage("RAW", None).is_err());
        assert!(validate_tier_lineage("Cleaned", Some(1)).is_err());
        assert!(validate_tier_lineage("Analytics", Some(1)).is_err());
    }

    #[test]
    fn all_valid_tier_lineage_combinations() {
        assert!(validate_tier_lineage("raw", None).is_ok());
        assert!(validate_tier_lineage("cleaned", Some(1)).is_ok());
        assert!(validate_tier_lineage("analytics", Some(2)).is_ok());
    }
}
