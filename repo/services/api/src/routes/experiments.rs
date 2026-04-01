use rocket::serde::json::Json;
use rocket::State;

use crate::contracts::{
    ApiError, ExperimentAssignRequest, ExperimentBacktrackRequest, ExperimentCreateRequest,
    ExperimentVariantRequest,
};
use crate::infrastructure::auth::middleware::CurrentUser;
use crate::AppState;

#[rocket::post("/api/v1/experiments", data = "<payload>")]
pub async fn create_experiment(
    state: &State<AppState>,
    user: CurrentUser,
    payload: Json<ExperimentCreateRequest>,
) -> Result<Json<i64>, ApiError> {
    Ok(Json(
        state
            .app_service
            .create_experiment(&user.0, payload.into_inner())
            .await?,
    ))
}

#[rocket::post("/api/v1/experiments/<experiment_id>/variants", data = "<payload>")]
pub async fn add_variant(
    state: &State<AppState>,
    user: CurrentUser,
    experiment_id: i64,
    payload: Json<ExperimentVariantRequest>,
) -> Result<Json<&'static str>, ApiError> {
    state
        .app_service
        .add_experiment_variant(&user.0, experiment_id, payload.into_inner())
        .await?;
    Ok(Json("added"))
}

#[rocket::post("/api/v1/experiments/<experiment_id>/assign", data = "<payload>")]
pub async fn assign_variant(
    state: &State<AppState>,
    user: CurrentUser,
    experiment_id: i64,
    payload: Json<ExperimentAssignRequest>,
) -> Result<Json<Option<String>>, ApiError> {
    Ok(Json(
        state
            .app_service
            .assign_experiment(&user.0, experiment_id, payload.into_inner())
            .await?,
    ))
}

#[rocket::post("/api/v1/experiments/<experiment_id>/backtrack", data = "<payload>")]
pub async fn backtrack(
    state: &State<AppState>,
    user: CurrentUser,
    experiment_id: i64,
    payload: Json<ExperimentBacktrackRequest>,
) -> Result<Json<&'static str>, ApiError> {
    state
        .app_service
        .backtrack_experiment(&user.0, experiment_id, payload.into_inner())
        .await?;
    Ok(Json("recorded"))
}
