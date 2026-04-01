pub use contracts::*;
use rocket::http::Status;
use rocket::response::Responder;
use rocket::serde::json::Json;
use rocket::{Request, Response};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("not found")]
    NotFound,
    #[error("conflict")]
    Conflict,
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("payload too large")]
    PayloadTooLarge,
    #[error("internal server error")]
    Internal,
}

impl ApiError {
    pub fn bad_request(message: &str) -> Self {
        Self::BadRequest(message.to_string())
    }
}

impl<'r> Responder<'r, 'static> for ApiError {
    fn respond_to(self, req: &'r Request<'_>) -> rocket::response::Result<'static> {
        let (status, code, message) = match self {
            ApiError::Database(_) | ApiError::Migrate(_) => (
                Status::ServiceUnavailable,
                "database_unavailable",
                "Database is not available",
            ),
            ApiError::Unauthorized => (Status::Unauthorized, "unauthorized", "Unauthorized"),
            ApiError::Forbidden => (Status::Forbidden, "forbidden", "Forbidden"),
            ApiError::NotFound => (Status::NotFound, "not_found", "Resource not found"),
            ApiError::Conflict => (Status::Conflict, "conflict", "Conflict"),
            ApiError::BadRequest(message) => return build_response(req, Status::BadRequest, "bad_request", &message),
            ApiError::PayloadTooLarge => (Status::PayloadTooLarge, "payload_too_large", "Payload too large"),
            ApiError::Internal => (
                Status::InternalServerError,
                "internal_error",
                "Unexpected internal error",
            ),
        };

        build_response(req, status, code, message)
    }
}

fn build_response<'r>(
    req: &'r Request<'_>,
    status: Status,
    code: &str,
    message: &str,
) -> rocket::response::Result<'static> {
    let payload = Json(ErrorResponse {
        code: code.to_string(),
        message: message.to_string(),
    });

    Response::build_from(payload.respond_to(req)?)
        .status(status)
        .ok()
}

#[derive(Debug, Clone)]
pub struct RetentionSnapshot {
    pub audit_log_days: u32,
    pub session_days: u32,
    pub patient_record_days: u32,
}

#[derive(Debug, Clone)]
pub struct SessionSnapshot {
    pub cookie_name: String,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: String,
    pub ttl_minutes: u32,
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: i64,
    pub username: String,
    pub role_name: String,
}
