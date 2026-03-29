use rocket::serde::json::Json;
use rocket::State;

use crate::contracts::SessionSettingsDto;
use crate::infrastructure::auth::middleware::CurrentUser;
use crate::AppState;

#[rocket::get("/api/v1/session")]
pub async fn session_settings(
    state: &State<AppState>,
    _user: CurrentUser,
) -> Json<SessionSettingsDto> {
    Json(SessionSettingsDto {
        cookie_name: state.session.cookie_name.clone(),
        secure: state.session.secure,
        http_only: state.session.http_only,
        same_site: state.session.same_site.clone(),
        ttl_minutes: state.session.ttl_minutes,
    })
}
