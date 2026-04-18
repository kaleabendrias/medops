# Audit Report 2 Fix Check

Scope: Static-only re-check of issues listed in .tmp/audit_report-2.md.
Boundary: No runtime execution, no Docker, no tests run.

## Overall Result
- Re-check outcome: All 5 previously listed issues are fixed in source and docs by static evidence.
- Remaining boundary: Runtime behavior (for example TLS handshake/cert trust in target environment) still requires manual verification.

## Issue-by-Issue Status

| Previous Issue | Previous Severity | Current Status | Static Evidence | Notes |
|---|---|---|---|---|
| Session bearer token was browser-readable in localStorage and reused in JS auth headers | High | Fixed | repo/services/web/src/state.rs:6, repo/services/web/src/state.rs:39, repo/services/web/src/api/mod.rs:73, repo/services/api/src/infrastructure/auth/middleware.rs:52, repo/services/api/src/routes/auth.rs:16 | Auth moved to HttpOnly cookie on server side. Frontend now stores csrf_token in session storage and sends X-CSRF-Token, not bearer auth token. |
| Default transport/session posture was non-secure (secure=false, plain HTTP) | High | Fixed | repo/services/api/config/default.toml:23, repo/services/web/nginx.conf:16, repo/services/web/nginx.conf:22, repo/docker-compose.yml:51, repo/README.md:9, repo/README.md:58 | Session secure=true now. HTTPS listener and TLS config are present. Public frontend docs now point to https on 8443. |
| Ingestion URL trust boundary enforced only at run-time (not create/update) | Medium | Fixed | repo/services/api/src/services/app_service/governance.rs:182, repo/services/api/src/services/app_service/governance.rs:205, repo/services/api/src/services/app_service/governance.rs:225, repo/API_tests/api_integration_tests.sh:830 | create_ingestion_task and update_ingestion_task now reject non-allowlisted seed URLs up front. Integration test expectation changed to 400 for external URL create attempt. |
| Documentation/contract drift (migration count and retention route contract mismatch) | Medium | Fixed | repo/README.md:10, repo/services/api/migrations/023_reveal_sensitive_entitlement.sql, docs/api-spec.md:145, repo/services/api/src/routes/retention.rs:29 | README migration count updated and matches current migration set (23). API spec retention route now matches implemented path with policy_key and years. |
| Frontend sensitive-reveal control hardcoded to role names | Low | Fixed | repo/services/web/src/features/session.rs:3, repo/services/web/src/features/session.rs:4, repo/services/web/src/features/session.rs:30 | reveal_sensitive logic is now entitlement-driven instead of role-name hardcoded. |

## Additional Static Notes
- Security model transition is consistent across backend and frontend:
  - Cookie-based session extraction in fairing: repo/services/api/src/infrastructure/auth/middleware.rs:52
  - Login sets session cookie: repo/services/api/src/routes/auth.rs:16
  - Mutating requests require CSRF header: repo/services/api/src/infrastructure/auth/middleware.rs:61
  - Frontend uses X-CSRF-Token helper: repo/services/web/src/api/mod.rs:73
- TLS support is implemented in web image build via self-signed cert generation:
  - repo/services/web/Dockerfile:36

## Final Re-check Verdict
- Fixed count: 5 of 5
- Not fixed: 0
- Cannot confirm statically: runtime deployment behavior only