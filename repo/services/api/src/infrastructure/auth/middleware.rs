use std::sync::Mutex;

use rocket::fairing::{Fairing, Info, Kind};
use rocket::request::{FromRequest, Outcome};
use rocket::{Data, Request};

use crate::contracts::{ApiError, AuthUser};
use crate::AppState;

pub struct AuthFairing;

#[rocket::async_trait]
impl Fairing for AuthFairing {
    fn info(&self) -> Info {
        Info {
            name: "Session auth and access logging",
            kind: Kind::Request,
        }
    }

    async fn on_request(&self, req: &mut Request<'_>, _data: &mut Data<'_>) {
        let path = req.uri().path().to_string();
        let correlation_id = uuid::Uuid::new_v4().to_string();
        req.local_cache(|| correlation_id.clone());

        let method = req.method().as_str().to_string();
        tracing::info!(
            category = "http",
            correlation_id = %correlation_id,
            method = %method,
            path = %path,
            "request_received"
        );

        if !path.starts_with("/api/v1") {
            return;
        }

        if matches!(path.as_str(), "/api/v1/health" | "/api/v1/auth/login") {
            return;
        }

        let cache = req.local_cache(|| Mutex::new(None::<AuthUser>));

        let token = match req.headers().get_one("X-Session-Token") {
            Some(value) if !value.trim().is_empty() => value.trim(),
            _ => return,
        };

        let state = match req.rocket().state::<AppState>() {
            Some(s) => s,
            None => return,
        };

        match state.app_service.validate_session_token(token).await {
            Ok(user) => {
                if let Ok(mut guard) = cache.lock() {
                    *guard = Some(user.clone());
                }
                let _ = state.app_service.append_access_audit(&user, &path).await;
            }
            Err(_) => {}
        }
    }
}

#[derive(Debug, Clone)]
pub struct CurrentUser(pub AuthUser);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for CurrentUser {
    type Error = ApiError;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let cache = req.local_cache(|| Mutex::new(None::<AuthUser>));
        let value = match cache.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => None,
        };

        match value {
            Some(user) => Outcome::Success(CurrentUser(user)),
            None => Outcome::Error((rocket::http::Status::Unauthorized, ApiError::Unauthorized)),
        }
    }
}
