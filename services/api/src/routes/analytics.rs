use rocket::serde::json::Json;
use rocket::State;

use crate::contracts::{ApiError, FunnelMetricsDto, RecommendationKpiDto, RetentionMetricsDto};
use crate::infrastructure::auth::middleware::CurrentUser;
use crate::AppState;

#[rocket::get("/api/v1/analytics/funnel")]
pub async fn funnel(
    state: &State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<FunnelMetricsDto>>, ApiError> {
    Ok(Json(state.app_service.funnel_metrics(&user.0).await?))
}

#[rocket::get("/api/v1/analytics/retention")]
pub async fn retention(
    state: &State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<RetentionMetricsDto>>, ApiError> {
    Ok(Json(state.app_service.retention_metrics(&user.0).await?))
}

#[rocket::get("/api/v1/analytics/recommendation-kpi")]
pub async fn recommendation_kpi(
    state: &State<AppState>,
    user: CurrentUser,
) -> Result<Json<RecommendationKpiDto>, ApiError> {
    Ok(Json(state.app_service.recommendation_kpi(&user.0).await?))
}
