use sha2::Digest;
use contracts::{AuthLoginRequest, AuthLoginResponse, HospitalDto, MenuEntitlementDto, RoleDto, UserSummaryDto};

use crate::contracts::{ApiError, AuthUser};
use super::AppService;

impl AppService {
    pub async fn list_hospitals(&self) -> Result<Vec<HospitalDto>, ApiError> {
        self.repo.list_hospitals().await
    }

    pub async fn list_roles(&self) -> Result<Vec<RoleDto>, ApiError> {
        self.repo.list_roles().await
    }

    pub(crate) fn csrf_token_for(bearer_token: &str) -> String {
        let input = format!("{bearer_token}:csrf-v1");
        hex::encode(sha2::Sha256::digest(input.as_bytes()))
    }

    pub async fn login(&self, req: AuthLoginRequest) -> Result<(String, AuthLoginResponse), ApiError> {
        if req.username.trim().is_empty() || req.password.is_empty() {
            Self::security_log(
                "auth.login",
                "rejected",
                serde_json::json!({"reason":"missing_credentials"}),
            );
            return Err(ApiError::bad_request("Username and password are required"));
        }

        let maybe_user = self.repo.get_user_auth(req.username.trim()).await?;
        let user = match maybe_user {
            Some(u) => u,
            None => {
                Self::security_log(
                    "auth.login",
                    "rejected",
                    serde_json::json!({"username":req.username.trim(),"reason":"unknown_user"}),
                );
                return Err(ApiError::Unauthorized);
            }
        };

        if user.disabled {
            Self::security_log(
                "auth.login",
                "rejected",
                serde_json::json!({"user_id":user.id,"reason":"disabled"}),
            );
            return Err(ApiError::Forbidden);
        }

        if user.locked_now {
            Self::security_log(
                "auth.login",
                "rejected",
                serde_json::json!({"user_id":user.id,"reason":"locked"}),
            );
            return Err(ApiError::bad_request(&format!(
                "Account is locked for {} minutes after failed logins",
                self.lockout_minutes
            )));
        }

        // Validate complexity AFTER user lookup and lockout check so that all
        // failed attempts — including non-complex passwords — count uniformly
        // toward the lockout threshold.
        if self.validate_password_complexity(&req.password).is_err() {
            self.repo
                .register_failed_login(user.id, user.failed_attempts + 1)
                .await?;
            Self::security_log(
                "auth.login",
                "rejected",
                serde_json::json!({"user_id":user.id,"reason":"non_complex_password"}),
            );
            return Err(ApiError::Unauthorized);
        }

        let verify = Self::verify_password(&req.password, &user.password_hash)?;
        if !verify.0 {
            self.repo
                .register_failed_login(user.id, user.failed_attempts + 1)
                .await?;
            Self::security_log(
                "auth.login",
                "rejected",
                serde_json::json!({"user_id":user.id,"reason":"bad_password"}),
            );
            return Err(ApiError::Unauthorized);
        }

        if verify.1 {
            let upgraded = Self::hash_password_argon2(&req.password)?;
            self.repo.update_user_password_hash(user.id, &upgraded).await?;
            Self::security_log(
                "auth.password_migration",
                "success",
                serde_json::json!({"user_id":user.id,"from":"legacy_sha256","to":"argon2id"}),
            );
        }

        self.repo.reset_login_failures(user.id).await?;
        let token = Self::generate_session_token();
        self.repo.create_session(&token, user.id).await?;
        self.repo
            .append_audit(
                "auth.login",
                "user",
                &user.id.to_string(),
                "{\"result\":\"success\"}",
                user.id,
            )
            .await?;

        Self::security_log(
            "auth.login",
            "success",
            serde_json::json!({"user_id":user.id,"role":user.role_name}),
        );

        let csrf_token = Self::csrf_token_for(&token);
        Ok((token, AuthLoginResponse {
            csrf_token,
            user_id: user.id,
            username: user.username,
            role: user.role_name,
            expires_in_minutes: self.session_inactivity_minutes,
        }))
    }

    pub async fn validate_session_token(&self, token: &str) -> Result<AuthUser, ApiError> {
        let session = self.repo.get_session(token).await?;
        let session = match session {
            Some(value) => value,
            None => {
                Self::security_log(
                    "auth.session_validate",
                    "rejected",
                    serde_json::json!({"reason":"missing_or_revoked","token_fingerprint":Self::token_fingerprint(token)}),
                );
                return Err(ApiError::Unauthorized);
            }
        };

        if session.disabled {
            Self::security_log(
                "auth.session_validate",
                "rejected",
                serde_json::json!({"user_id":session.user_id,"reason":"disabled"}),
            );
            return Err(ApiError::Forbidden);
        }
        if session.inactive_expired {
            Self::security_log(
                "auth.session_validate",
                "rejected",
                serde_json::json!({"user_id":session.user_id,"reason":"inactivity_expired"}),
            );
            return Err(ApiError::Unauthorized);
        }

        self.repo.touch_session(token).await?;

        Ok(AuthUser {
            user_id: session.user_id,
            username: session.username,
            role_name: session.role_name,
        })
    }

    pub async fn authorize(&self, user: &AuthUser, permission: &str) -> Result<(), ApiError> {
        let allowed = self
            .repo
            .user_has_permission(&user.role_name, permission)
            .await?;
        if !allowed {
            Self::security_log(
                "auth.permission",
                "rejected",
                serde_json::json!({"user_id":user.user_id,"role":user.role_name,"permission":permission}),
            );
            return Err(ApiError::Forbidden);
        }
        Ok(())
    }

    pub async fn menu_entitlements(&self, user: &AuthUser) -> Result<Vec<MenuEntitlementDto>, ApiError> {
        self.repo.list_menu_entitlements(&user.role_name).await
    }

    pub async fn list_users(&self, user: &AuthUser) -> Result<Vec<UserSummaryDto>, ApiError> {
        self.authorize(user, "admin.disable_user").await?;
        self.repo.list_users().await
    }

    pub async fn disable_user(&self, user: &AuthUser, target_user_id: i64) -> Result<(), ApiError> {
        self.authorize(user, "admin.disable_user").await?;
        self.repo.disable_user(target_user_id).await?;
        self.repo.revoke_user_sessions(target_user_id).await?;
        self.repo
            .append_audit(
                "admin.disable_user",
                "user",
                &target_user_id.to_string(),
                "{\"immediate\":true}",
                user.user_id,
            )
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::AppService;

    #[test]
    fn csrf_token_is_64_hex_chars() {
        let token = AppService::csrf_token_for("somebearer");
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn csrf_token_is_deterministic() {
        let t1 = AppService::csrf_token_for("abc123");
        let t2 = AppService::csrf_token_for("abc123");
        assert_eq!(t1, t2);
    }

    #[test]
    fn csrf_token_differs_per_bearer() {
        let t1 = AppService::csrf_token_for("token_a");
        let t2 = AppService::csrf_token_for("token_b");
        assert_ne!(t1, t2);
    }

    #[test]
    fn csrf_token_known_vector() {
        use sha2::Digest;
        // SHA256("test:csrf-v1") = known hex; verifies the derivation formula
        let expected = hex::encode(sha2::Sha256::digest(b"test:csrf-v1"));
        assert_eq!(AppService::csrf_token_for("test"), expected);
    }
}
