use rocket::serde::json::Json;
use rocket::State;

use crate::contracts::{ApiError, CampaignCreateRequest, CampaignDto};
use crate::infrastructure::auth::middleware::CurrentUser;
use crate::AppState;

#[rocket::post("/api/v1/campaigns", data = "<payload>")]
pub async fn create_campaign(
    state: &State<AppState>,
    user: CurrentUser,
    payload: Json<CampaignCreateRequest>,
) -> Result<Json<i64>, ApiError> {
    Ok(Json(
        state
            .app_service
            .create_campaign(&user.0, payload.into_inner())
            .await?,
    ))
}

#[rocket::post("/api/v1/campaigns/<campaign_id>/join")]
pub async fn join_campaign(
    state: &State<AppState>,
    user: CurrentUser,
    campaign_id: i64,
) -> Result<Json<&'static str>, ApiError> {
    state.app_service.join_campaign(&user.0, campaign_id).await?;
    Ok(Json("joined"))
}

#[rocket::get("/api/v1/campaigns")]
pub async fn list_campaigns(
    state: &State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<CampaignDto>>, ApiError> {
    Ok(Json(state.app_service.campaigns(&user.0).await?))
}
