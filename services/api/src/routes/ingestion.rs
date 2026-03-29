use rocket::serde::json::Json;
use rocket::State;

use crate::contracts::{
    ApiError, IngestionTaskCreateRequest, IngestionTaskDto, IngestionTaskRollbackRequest,
    IngestionTaskRunDto, IngestionTaskUpdateRequest, IngestionTaskVersionDto,
};
use crate::infrastructure::auth::middleware::CurrentUser;
use crate::AppState;

#[rocket::post("/api/v1/ingestion/tasks", data = "<payload>")]
pub async fn create_task(
    state: &State<AppState>,
    user: CurrentUser,
    payload: Json<IngestionTaskCreateRequest>,
) -> Result<Json<i64>, ApiError> {
    Ok(Json(
        state
            .app_service
            .create_ingestion_task(&user.0, payload.into_inner())
            .await?,
    ))
}

#[rocket::put("/api/v1/ingestion/tasks/<task_id>", data = "<payload>")]
pub async fn update_task(
    state: &State<AppState>,
    user: CurrentUser,
    task_id: i64,
    payload: Json<IngestionTaskUpdateRequest>,
) -> Result<Json<i32>, ApiError> {
    Ok(Json(
        state
            .app_service
            .update_ingestion_task(&user.0, task_id, payload.into_inner())
            .await?,
    ))
}

#[rocket::post("/api/v1/ingestion/tasks/<task_id>/rollback", data = "<payload>")]
pub async fn rollback_task(
    state: &State<AppState>,
    user: CurrentUser,
    task_id: i64,
    payload: Json<IngestionTaskRollbackRequest>,
) -> Result<Json<i32>, ApiError> {
    Ok(Json(
        state
            .app_service
            .rollback_ingestion_task(&user.0, task_id, payload.into_inner())
            .await?,
    ))
}

#[rocket::post("/api/v1/ingestion/tasks/<task_id>/run")]
pub async fn run_task(
    state: &State<AppState>,
    user: CurrentUser,
    task_id: i64,
) -> Result<Json<&'static str>, ApiError> {
    state.app_service.run_ingestion_task(&user.0, task_id).await?;
    Ok(Json("started"))
}

#[rocket::get("/api/v1/ingestion/tasks")]
pub async fn list_tasks(
    state: &State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<IngestionTaskDto>>, ApiError> {
    Ok(Json(state.app_service.list_ingestion_tasks(&user.0).await?))
}

#[rocket::get("/api/v1/ingestion/tasks/<task_id>/versions")]
pub async fn task_versions(
    state: &State<AppState>,
    user: CurrentUser,
    task_id: i64,
) -> Result<Json<Vec<IngestionTaskVersionDto>>, ApiError> {
    Ok(Json(
        state
            .app_service
            .ingestion_task_versions(&user.0, task_id)
            .await?,
    ))
}

#[rocket::get("/api/v1/ingestion/tasks/<task_id>/runs?<limit>")]
pub async fn task_runs(
    state: &State<AppState>,
    user: CurrentUser,
    task_id: i64,
    limit: Option<i64>,
) -> Result<Json<Vec<IngestionTaskRunDto>>, ApiError> {
    Ok(Json(
        state
            .app_service
            .ingestion_task_runs(&user.0, task_id, limit.unwrap_or(20))
            .await?,
    ))
}
