use rocket::serde::json::Json;
use rocket::State;

use crate::contracts::{ApiError, TelemetryEventRequest};
use crate::infrastructure::auth::middleware::CurrentUser;
use crate::AppState;

#[rocket::post("/api/v1/telemetry/events", data = "<payload>")]
pub async fn create_event(
    state: &State<AppState>,
    user: CurrentUser,
    payload: Json<TelemetryEventRequest>,
) -> Result<Json<&'static str>, ApiError> {
    state
        .app_service
        .telemetry_event(&user.0, payload.into_inner())
        .await?;
    Ok(Json("accepted"))
}
