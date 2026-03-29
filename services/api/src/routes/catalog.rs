use rocket::serde::json::Json;
use rocket::State;

use crate::contracts::{ApiError, HospitalDto, RoleDto};
use crate::infrastructure::auth::middleware::CurrentUser;
use crate::AppState;

#[rocket::get("/api/v1/hospitals")]
pub async fn hospitals(state: &State<AppState>, user: CurrentUser) -> Result<Json<Vec<HospitalDto>>, ApiError> {
    state.app_service.authorize(&user.0, "catalog.read").await?;
    Ok(Json(state.app_service.list_hospitals().await?))
}

#[rocket::get("/api/v1/roles")]
pub async fn roles(state: &State<AppState>, user: CurrentUser) -> Result<Json<Vec<RoleDto>>, ApiError> {
    state.app_service.authorize(&user.0, "catalog.read").await?;
    Ok(Json(state.app_service.list_roles().await?))
}
