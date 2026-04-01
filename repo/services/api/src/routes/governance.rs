use rocket::serde::json::Json;
use rocket::State;

use crate::contracts::{
    ApiError, GovernanceDeleteRequest, GovernanceRecordDto, GovernanceRecordRequest,
};
use crate::infrastructure::auth::middleware::CurrentUser;
use crate::AppState;

#[rocket::post("/api/v1/governance/records", data = "<payload>")]
pub async fn create_record(
    state: &State<AppState>,
    user: CurrentUser,
    payload: Json<GovernanceRecordRequest>,
) -> Result<Json<i64>, ApiError> {
    let id = state
        .app_service
        .create_governance_record(&user.0, payload.into_inner())
        .await?;
    Ok(Json(id))
}

#[rocket::get("/api/v1/governance/records")]
pub async fn list_records(
    state: &State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<GovernanceRecordDto>>, ApiError> {
    Ok(Json(state.app_service.list_governance_records(&user.0).await?))
}

#[rocket::delete("/api/v1/governance/records/<record_id>", data = "<payload>")]
pub async fn tombstone_record(
    state: &State<AppState>,
    user: CurrentUser,
    record_id: i64,
    payload: Json<GovernanceDeleteRequest>,
) -> Result<Json<&'static str>, ApiError> {
    state
        .app_service
        .tombstone_governance_record(&user.0, record_id, payload.into_inner())
        .await?;
    Ok(Json("tombstoned"))
}
