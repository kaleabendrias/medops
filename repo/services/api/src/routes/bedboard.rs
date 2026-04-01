use rocket::serde::json::Json;
use rocket::State;

use crate::contracts::{ApiError, BedDto, BedEventDto, BedTransitionRequest};
use crate::infrastructure::auth::middleware::CurrentUser;
use crate::AppState;

#[rocket::get("/api/v1/bedboard/beds")]
pub async fn list_beds(
    state: &State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<BedDto>>, ApiError> {
    Ok(Json(state.app_service.list_beds(&user.0).await?))
}

#[rocket::post("/api/v1/bedboard/beds/<bed_id>/transition", data = "<payload>")]
pub async fn transition(
    state: &State<AppState>,
    user: CurrentUser,
    bed_id: i64,
    payload: Json<BedTransitionRequest>,
) -> Result<Json<&'static str>, ApiError> {
    state
        .app_service
        .transition_bed(&user.0, bed_id, payload.into_inner())
        .await?;
    Ok(Json("transitioned"))
}

#[rocket::get("/api/v1/bedboard/events")]
pub async fn events(
    state: &State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<BedEventDto>>, ApiError> {
    Ok(Json(state.app_service.bed_events(&user.0).await?))
}
