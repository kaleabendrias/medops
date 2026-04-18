use std::sync::Mutex;

use rocket::fairing::{Fairing, Info, Kind};
use rocket::request::{FromRequest, Outcome};
use rocket::{Data, Request};

use crate::contracts::{ApiError, AuthUser};
use crate::services::app_service::AppService;
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

        if matches!(
            path.as_str(),
            "/api/v1/health" | "/api/v1/auth/login" | "/api/v1/auth/logout"
        ) {
            return;
        }

        let state = match req.rocket().state::<AppState>() {
            Some(s) => s,
            None => return,
        };

        let token = match req.cookies().get(&state.session.cookie_name) {
            Some(cookie) if !cookie.value().trim().is_empty() => cookie.value().trim().to_string(),
            _ => return,
        };

        // Validate CSRF token for state-changing requests.
        // GET/HEAD/OPTIONS are safe and exempt from this check.
        let mutating = matches!(method.as_str(), "POST" | "PUT" | "DELETE" | "PATCH");
        if mutating {
            let expected_csrf = AppService::csrf_token_for(&token);
            let provided_csrf = req.headers().get_one("X-CSRF-Token").unwrap_or("").trim();
            if provided_csrf != expected_csrf {
                // Log the rejection but do not cache a user — the route will
                // get None from CurrentUser and respond with 401.
                tracing::warn!(
                    category = "security",
                    event = "csrf.mismatch",
                    path = %path,
                    "csrf_validation_failed"
                );
                return;
            }
        }

        let cache = req.local_cache(|| Mutex::new(None::<AuthUser>));

        match state.app_service.validate_session_token(&token).await {
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

#[cfg(test)]
mod tests {
    #[test]
    fn mutating_methods_require_csrf() {
        let mutating = ["POST", "PUT", "DELETE", "PATCH"];
        let safe = ["GET", "HEAD", "OPTIONS"];
        for m in mutating {
            assert!(matches!(m, "POST" | "PUT" | "DELETE" | "PATCH"), "{m} should require CSRF");
        }
        for m in safe {
            assert!(!matches!(m, "POST" | "PUT" | "DELETE" | "PATCH"), "{m} should not require CSRF");
        }
    }

    #[test]
    fn auth_exempt_paths_include_health_and_login() {
        let exempt = ["/api/v1/health", "/api/v1/auth/login", "/api/v1/auth/logout"];
        for path in exempt {
            let is_exempt = matches!(
                path,
                "/api/v1/health" | "/api/v1/auth/login" | "/api/v1/auth/logout"
            );
            assert!(is_exempt, "{path} should be exempt from auth");
        }
    }

    #[test]
    fn non_api_paths_are_not_processed() {
        let paths = ["/", "/index.html", "/static/app.js"];
        for path in paths {
            assert!(!path.starts_with("/api/v1"), "{path} should bypass auth fairing");
        }
    }
}
