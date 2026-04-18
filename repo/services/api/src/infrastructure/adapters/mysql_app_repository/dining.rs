use contracts::{
    CampaignDto, DiningMenuDto, DishCategoryDto, DishDto, OrderDto, OrderNoteDto, RankingRuleDto,
    RecommendationDto, TicketSplitDto,
};

use crate::contracts::ApiError;
use crate::repositories::app_repository::{AppRepository, OrderRecord};
use super::MySqlAppRepository;

impl MySqlAppRepository {
    pub(super) async fn create_menu_impl(&self, menu_date: &str, meal_period: &str, item_name: &str, calories: i32, actor_id: i64) -> Result<(), ApiError> {
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

    pub(super) async fn list_menus_impl(&self) -> Result<Vec<DiningMenuDto>, ApiError> {
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

    pub(super) async fn validate_menu_orderable_impl(&self, menu_id: i64) -> Result<(), ApiError> {
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

    pub(super) async fn create_order_idempotent_impl(
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

    pub(super) async fn get_order_impl(&self, order_id: i64) -> Result<Option<OrderRecord>, ApiError> {
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

    pub(super) async fn set_order_status_if_version_impl(
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

    pub(super) async fn add_order_note_impl(&self, order_id: i64, note: &str, staff_user_id: i64) -> Result<(), ApiError> {
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

    pub(super) async fn list_order_notes_impl(&self, order_id: i64) -> Result<Vec<OrderNoteDto>, ApiError> {
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

    pub(super) async fn add_ticket_split_impl(&self, order_id: i64, split_by: &str, split_value: &str, quantity: i32) -> Result<(), ApiError> {
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

    pub(super) async fn list_ticket_splits_impl(&self, order_id: i64) -> Result<Vec<TicketSplitDto>, ApiError> {
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

    pub(super) async fn list_dish_categories_impl(&self) -> Result<Vec<DishCategoryDto>, ApiError> {
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

    pub(super) async fn create_dish_impl(
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

    pub(super) async fn list_dishes_impl(&self) -> Result<Vec<DishDto>, ApiError> {
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

    pub(super) async fn set_dish_status_impl(&self, dish_id: i64, is_published: bool, is_sold_out: bool) -> Result<(), ApiError> {
        sqlx::query("UPDATE dishes SET is_published = ?, is_sold_out = ?, updated_at = NOW() WHERE id = ?")
            .bind(is_published)
            .bind(is_sold_out)
            .bind(dish_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub(super) async fn add_dish_option_impl(&self, dish_id: i64, option_group: &str, option_value: &str, delta_price_cents: i32) -> Result<(), ApiError> {
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

    pub(super) async fn add_sales_window_impl(&self, dish_id: i64, slot_name: &str, start_hhmm: &str, end_hhmm: &str) -> Result<(), ApiError> {
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

    pub(super) async fn upsert_ranking_rule_impl(&self, rule_key: &str, weight: f64, enabled: bool, actor_id: i64) -> Result<(), ApiError> {
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

    pub(super) async fn list_ranking_rules_impl(&self) -> Result<Vec<RankingRuleDto>, ApiError> {
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

    pub(super) async fn recommendations_impl(&self) -> Result<Vec<RecommendationDto>, ApiError> {
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
                    CAST(SUM(CASE WHEN te.event_name = 'recommendation_click' THEN 1 ELSE 0 END) AS SIGNED) AS clicks,
                    CAST(SUM(CASE WHEN te.event_name = 'order_created' THEN 1 ELSE 0 END) AS SIGNED) AS conversions
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

    pub(super) async fn create_campaign_impl(
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

    pub(super) async fn join_campaign_impl(&self, campaign_id: i64, user_id: i64) -> Result<(), ApiError> {
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

    pub(super) async fn list_campaigns_impl(&self) -> Result<Vec<CampaignDto>, ApiError> {
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

    pub(super) async fn close_inactive_campaigns_impl(&self) -> Result<(), ApiError> {
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

    pub(super) async fn list_orders_impl(&self, user_id: i64, role_name: &str, limit: i64, offset: i64) -> Result<Vec<OrderDto>, ApiError> {
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
}
