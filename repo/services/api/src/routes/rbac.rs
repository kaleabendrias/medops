use rocket::serde::json::Json;
use rocket::State;

use crate::contracts::{ApiError, MenuEntitlementDto, UserSummaryDto};
use crate::infrastructure::auth::middleware::CurrentUser;
use crate::AppState;

#[rocket::get("/api/v1/rbac/menu-entitlements")]
pub async fn menu_entitlements(
    state: &State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<MenuEntitlementDto>>, ApiError> {
    let items = state.app_service.menu_entitlements(&user.0).await?;
    Ok(Json(items))
}

#[rocket::get("/api/v1/admin/users")]
pub async fn list_users(
    state: &State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<UserSummaryDto>>, ApiError> {
    let users = state.app_service.list_users(&user.0).await?;
    Ok(Json(users))
}

#[rocket::post("/api/v1/admin/users/<user_id>/disable")]
pub async fn disable_user(
    state: &State<AppState>,
    user: CurrentUser,
    user_id: i64,
) -> Result<Json<&'static str>, ApiError> {
    state.app_service.disable_user(&user.0, user_id).await?;
    Ok(Json("disabled"))
}
