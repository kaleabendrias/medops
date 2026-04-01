use rocket::serde::json::Json;
use rocket::State;

use crate::contracts::{ApiError, AuditLogDto};
use crate::infrastructure::auth::middleware::CurrentUser;
use crate::AppState;

#[rocket::get("/api/v1/audits")]
pub async fn list_audits(
    state: &State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<AuditLogDto>>, ApiError> {
    Ok(Json(state.app_service.list_audits(&user.0).await?))
}

#[rocket::put("/api/v1/audits")]
pub async fn reject_audit_update(_user: CurrentUser) -> Result<Json<&'static str>, ApiError> {
    Err(ApiError::bad_request("audit logs are append-only; updates are rejected"))
}

#[rocket::delete("/api/v1/audits")]
pub async fn reject_audit_delete(_user: CurrentUser) -> Result<Json<&'static str>, ApiError> {
    Err(ApiError::bad_request("audit logs are append-only; deletes are rejected"))
}
