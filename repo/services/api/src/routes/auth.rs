use rocket::http::{Cookie, CookieJar, SameSite};
use rocket::serde::json::Json;
use rocket::State;

use crate::contracts::{ApiError, AuthLoginRequest, AuthLoginResponse};
use crate::repositories::app_repository::AppRepository;
use crate::AppState;

#[rocket::post("/api/v1/auth/login", data = "<payload>")]
pub async fn login(
    state: &State<AppState>,
    cookies: &CookieJar<'_>,
    payload: Json<AuthLoginRequest>,
) -> Result<Json<AuthLoginResponse>, ApiError> {
    let (bearer_token, response) = state.app_service.login(payload.into_inner()).await?;
    let cookie = Cookie::build((state.session.cookie_name.clone(), bearer_token))
        .http_only(state.session.http_only)
        .secure(state.session.secure)
        .same_site(SameSite::Lax)
        .path("/");
    cookies.add(cookie);
    Ok(Json(response))
}

#[rocket::post("/api/v1/auth/logout")]
pub async fn logout(
    state: &State<AppState>,
    cookies: &CookieJar<'_>,
) -> Result<(), ApiError> {
    if let Some(cookie) = cookies.get(&state.session.cookie_name) {
        let token = cookie.value().to_string();
        let _ = state.app_service.repo.delete_session(&token).await;
        cookies.remove(Cookie::build(state.session.cookie_name.clone()).path("/"));
    }
    Ok(())
}
