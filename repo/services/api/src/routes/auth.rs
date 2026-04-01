use rocket::serde::json::Json;
use rocket::State;

use crate::contracts::{ApiError, AuthLoginRequest, AuthLoginResponse};
use crate::AppState;

#[rocket::post("/api/v1/auth/login", data = "<payload>")]
pub async fn login(
    state: &State<AppState>,
    payload: Json<AuthLoginRequest>,
) -> Result<Json<AuthLoginResponse>, ApiError> {
    let response = state.app_service.login(payload.into_inner()).await?;
    Ok(Json(response))
}
