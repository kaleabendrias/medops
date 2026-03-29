use rocket::serde::json::Json;
use rocket::State;

use crate::contracts::{ApiError, RetentionPolicyDto, RetentionSettingsDto};
use crate::infrastructure::auth::middleware::CurrentUser;
use crate::AppState;

#[rocket::get("/api/v1/retention")]
pub async fn retention_settings(
    state: &State<AppState>,
    _user: CurrentUser,
) -> Json<RetentionSettingsDto> {
    Json(RetentionSettingsDto {
        audit_log_days: state.retention.audit_log_days,
        session_days: state.retention.session_days,
        patient_record_days: state.retention.patient_record_days,
    })
}

#[rocket::get("/api/v1/retention/policies")]
pub async fn retention_policies(
    state: &State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<RetentionPolicyDto>>, ApiError> {
    let items = state.app_service.list_retention_policies(&user.0).await?;
    Ok(Json(items))
}

#[rocket::put("/api/v1/retention/policies/<policy_key>/<years>")]
pub async fn upsert_retention_policy(
    state: &State<AppState>,
    user: CurrentUser,
    policy_key: &str,
    years: i32,
) -> Result<Json<&'static str>, ApiError> {
    state
        .app_service
        .set_retention_policy(&user.0, policy_key, years)
        .await?;
    Ok(Json("updated"))
}
