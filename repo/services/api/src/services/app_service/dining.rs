use contracts::{
    CampaignCreateRequest, CampaignDto, DiningMenuDto, DiningMenuRequest, DishCategoryDto,
    DishCreateRequest, DishDto, DishOptionRequest, DishStatusRequest, DishWindowRequest,
    OrderCreateRequest, OrderDto, OrderNoteDto, OrderNoteRequest, OrderStatusRequest,
    RankingRuleDto, RankingRuleRequest, RecommendationDto, TicketSplitDto, TicketSplitRequest,
};

use crate::contracts::{ApiError, AuthUser};
use crate::repositories::app_repository::OrderRecord;
use super::AppService;

impl AppService {
    pub async fn list_menus(&self, user: &AuthUser) -> Result<Vec<DiningMenuDto>, ApiError> {
        self.authorize(user, "dining.read").await?;
        self.repo.list_menus().await
    }

    pub async fn create_menu(&self, user: &AuthUser, req: DiningMenuRequest) -> Result<(), ApiError> {
        self.authorize(user, "dining.write").await?;
        self.repo
            .create_menu(
                req.menu_date.trim(),
                req.meal_period.trim(),
                req.item_name.trim(),
                req.calories,
                user.user_id,
            )
            .await
    }

    pub async fn place_order(&self, user: &AuthUser, req: OrderCreateRequest) -> Result<i64, ApiError> {
        self.authorize(user, "order.write").await?;
        let has_global_order_access = self
            .repo
            .user_has_permission(&user.role_name, "order.global_access")
            .await?;
        let patient_access = if has_global_order_access {
            true
        } else {
            self.repo
                .can_access_patient(user.user_id, &user.role_name, req.patient_id)
                .await?
        };
        if !patient_access {
            return Err(ApiError::NotFound);
        }
        // Strict pre-flight menu governance check. The repository confirms
        // that the menu line is for today, that the linked dish is published,
        // not sold out, and that the current server time falls inside an
        // active sales window. Any failure aborts the order before it ever
        // touches dining_orders, so neither staff nor self-service members
        // can bypass dining availability rules.
        self.repo.validate_menu_orderable(req.menu_id).await?;
        let order_id = self
            .repo
            .create_order_idempotent(
                req.patient_id,
                req.menu_id,
                req.notes.trim(),
                user.user_id,
                req.idempotency_key.as_deref(),
            )
            .await?;
        self.repo
            .append_audit(
                "order.create",
                "dining_order",
                &order_id.to_string(),
                "{\"status\":\"Created\"}",
                user.user_id,
            )
            .await?;
        self.repo.close_inactive_campaigns().await?;
        Ok(order_id)
    }

    pub async fn set_order_status(&self, user: &AuthUser, order_id: i64, req: OrderStatusRequest) -> Result<(), ApiError> {
        self.authorize(user, "order.write").await?;
        let status = req.status.trim();
        let valid = ["Created", "Billed", "Canceled", "Credited"];
        if !valid.contains(&status) {
            return Err(ApiError::bad_request("Invalid order status"));
        }

        let current = self.repo.get_order(order_id).await?.ok_or(ApiError::NotFound)?;
        self.ensure_order_access(user, &current).await?;

        let transition_ok = match (current.status.as_str(), status) {
            ("Created", "Billed") => true,
            ("Created", "Canceled") => true,
            ("Billed", "Credited") => true,
            (a, b) if a == b => true,
            _ => false,
        };
        if !transition_ok {
            return Err(ApiError::bad_request("Invalid order transition"));
        }
        if Self::status_requires_reason(status)
            && req.reason.as_deref().unwrap_or(" ").trim().is_empty()
        {
            return Err(ApiError::bad_request(
                "Reason is required when canceling or crediting an order",
            ));
        }

        let expected = req.expected_version.unwrap_or(current.version);
        let changed = self
            .repo
            .set_order_status_if_version(order_id, expected, status, req.reason.as_deref())
            .await?;
        if !changed {
            return Err(ApiError::Conflict);
        }

        self.repo
            .append_audit(
                "order.status",
                "dining_order",
                &order_id.to_string(),
                &format!("{{\"status\":{}}}", serde_json::to_string(status).map_err(|_| ApiError::Internal)?),
                user.user_id,
            )
            .await?;
        self.repo.close_inactive_campaigns().await?;
        Ok(())
    }

    pub async fn list_orders(&self, user: &AuthUser, limit: i64, offset: i64) -> Result<Vec<OrderDto>, ApiError> {
        self.authorize(user, "order.read").await?;
        self.repo
            .list_orders(user.user_id, &user.role_name, limit, offset)
            .await
    }

    pub async fn list_dish_categories(&self, user: &AuthUser) -> Result<Vec<DishCategoryDto>, ApiError> {
        self.authorize(user, "dining.read").await?;
        self.repo.list_dish_categories().await
    }

    pub async fn create_dish(&self, user: &AuthUser, req: DishCreateRequest) -> Result<i64, ApiError> {
        self.authorize(user, "dining.write").await?;
        if req.name.trim().is_empty() {
            return Err(ApiError::bad_request("Dish name is required"));
        }
        let id = self
            .repo
            .create_dish(
                req.category_id,
                req.name.trim(),
                req.description.trim(),
                req.base_price_cents,
                req.photo_path.trim(),
                user.user_id,
            )
            .await?;
        Ok(id)
    }

    pub async fn list_dishes(&self, user: &AuthUser) -> Result<Vec<DishDto>, ApiError> {
        self.authorize(user, "dining.read").await?;
        self.repo.list_dishes().await
    }

    pub async fn set_dish_status(&self, user: &AuthUser, dish_id: i64, req: DishStatusRequest) -> Result<(), ApiError> {
        self.authorize(user, "dining.write").await?;
        self.repo
            .set_dish_status(dish_id, req.is_published, req.is_sold_out)
            .await
    }

    pub async fn add_dish_option(&self, user: &AuthUser, dish_id: i64, req: DishOptionRequest) -> Result<(), ApiError> {
        self.authorize(user, "dining.write").await?;
        self.repo
            .add_dish_option(
                dish_id,
                req.option_group.trim(),
                req.option_value.trim(),
                req.delta_price_cents,
            )
            .await
    }

    pub async fn add_sales_window(&self, user: &AuthUser, dish_id: i64, req: DishWindowRequest) -> Result<(), ApiError> {
        self.authorize(user, "dining.write").await?;
        self.repo
            .add_sales_window(
                dish_id,
                req.slot_name.trim(),
                req.start_hhmm.trim(),
                req.end_hhmm.trim(),
            )
            .await
    }

    pub async fn upsert_ranking_rule(&self, user: &AuthUser, req: RankingRuleRequest) -> Result<(), ApiError> {
        self.authorize(user, "dining.write").await?;
        self.repo
            .upsert_ranking_rule(req.rule_key.trim(), req.weight, req.enabled, user.user_id)
            .await
    }

    pub async fn ranking_rules(&self, user: &AuthUser) -> Result<Vec<RankingRuleDto>, ApiError> {
        self.authorize(user, "dining.read").await?;
        self.repo.list_ranking_rules().await
    }

    pub async fn recommendations(&self, user: &AuthUser) -> Result<Vec<RecommendationDto>, ApiError> {
        self.authorize(user, "dining.read").await?;
        self.repo.recommendations().await
    }

    pub async fn create_campaign(&self, user: &AuthUser, req: CampaignCreateRequest) -> Result<i64, ApiError> {
        self.authorize(user, "order.write").await?;
        if req.success_threshold <= 0 {
            return Err(ApiError::bad_request("success_threshold must be greater than 0"));
        }
        let deadline = Self::normalize_campaign_deadline(&req.success_deadline_at)?;
        self.repo.close_inactive_campaigns().await?;
        self.repo
            .create_campaign(
                req.title.trim(),
                req.dish_id,
                req.success_threshold,
                &deadline,
                user.user_id,
            )
            .await
    }

    pub async fn join_campaign(&self, user: &AuthUser, campaign_id: i64) -> Result<(), ApiError> {
        self.authorize(user, "order.write").await?;
        self.repo.close_inactive_campaigns().await?;
        self.repo.join_campaign(campaign_id, user.user_id).await
    }

    pub async fn campaigns(&self, user: &AuthUser) -> Result<Vec<CampaignDto>, ApiError> {
        self.authorize(user, "dining.read").await?;
        self.repo.close_inactive_campaigns().await?;
        self.repo.list_campaigns().await
    }

    pub async fn add_ticket_split(&self, user: &AuthUser, order_id: i64, req: TicketSplitRequest) -> Result<(), ApiError> {
        self.authorize(user, "order.write").await?;
        let order = self.repo.get_order(order_id).await?.ok_or(ApiError::NotFound)?;
        self.ensure_order_access(user, &order).await?;
        const VALID_SPLIT_BY: [&str; 2] = ["pickup_point", "kitchen_station"];
        if !VALID_SPLIT_BY.contains(&req.split_by.trim()) {
            return Err(ApiError::bad_request("split_by must be pickup_point or kitchen_station"));
        }
        if req.quantity <= 0 {
            return Err(ApiError::bad_request("quantity must be greater than zero"));
        }
        self.repo
            .add_ticket_split(order_id, req.split_by.trim(), req.split_value.trim(), req.quantity)
            .await?;
        self.repo
            .append_audit(
                "order.ticket_split",
                "dining_order",
                &order_id.to_string(),
                &format!(
                    "{{\"split_by\":{},\"split_value\":{},\"quantity\":{}}}",
                    serde_json::to_string(req.split_by.trim()).map_err(|_| ApiError::Internal)?,
                    serde_json::to_string(req.split_value.trim()).map_err(|_| ApiError::Internal)?,
                    req.quantity
                ),
                user.user_id,
            )
            .await?;
        Ok(())
    }

    pub async fn list_ticket_splits(&self, user: &AuthUser, order_id: i64) -> Result<Vec<TicketSplitDto>, ApiError> {
        self.authorize(user, "order.read").await?;
        let order = self.repo.get_order(order_id).await?.ok_or(ApiError::NotFound)?;
        self.ensure_order_access(user, &order).await?;
        self.repo.list_ticket_splits(order_id).await
    }

    pub async fn add_order_note(&self, user: &AuthUser, order_id: i64, req: OrderNoteRequest) -> Result<(), ApiError> {
        self.authorize(user, "order.write").await?;
        let order = self.repo.get_order(order_id).await?.ok_or(ApiError::NotFound)?;
        self.ensure_order_access(user, &order).await?;
        if req.note.trim().is_empty() {
            return Err(ApiError::bad_request("Order note cannot be empty"));
        }
        self.repo
            .add_order_note(order_id, req.note.trim(), user.user_id)
            .await?;
        self.repo
            .append_audit(
                "order.note",
                "dining_order",
                &order_id.to_string(),
                &format!(
                    "{{\"note_preview\":{}}}",
                    serde_json::to_string(&req.note.trim().chars().take(80).collect::<String>())
                        .map_err(|_| ApiError::Internal)?
                ),
                user.user_id,
            )
            .await?;
        Ok(())
    }

    pub async fn order_notes(&self, user: &AuthUser, order_id: i64) -> Result<Vec<OrderNoteDto>, ApiError> {
        self.authorize(user, "order.read").await?;
        let order = self.repo.get_order(order_id).await?.ok_or(ApiError::NotFound)?;
        self.ensure_order_access(user, &order).await?;
        self.repo.list_order_notes(order_id).await
    }

    pub(crate) async fn ensure_order_access(&self, user: &AuthUser, order: &OrderRecord) -> Result<(), ApiError> {
        let has_global_order_access = self
            .repo
            .user_has_permission(&user.role_name, "order.global_access")
            .await?;
        if has_global_order_access {
            return Ok(());
        }
        // Self-service users can only access orders they created themselves.
        let has_self_service = self
            .repo
            .user_has_permission(&user.role_name, "order.self_service")
            .await?;
        if has_self_service {
            if order.created_by == user.user_id {
                return Ok(());
            }
            return Err(ApiError::NotFound);
        }
        let can_access = self
            .repo
            .can_access_patient(user.user_id, &user.role_name, order.patient_id)
            .await?;
        if !can_access {
            return Err(ApiError::NotFound);
        }
        Ok(())
    }
}
