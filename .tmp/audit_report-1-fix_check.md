# Fix Check Report: audit_report-1

## 1. Scope and Boundary
- Check type: Static-only re-check against issues listed in .tmp/audit_report-1.md.
- Not performed: Runtime execution, Docker startup, test execution.
- Result confidence: Code/documentation consistency is verifiable statically; runtime behavior remains manual verification scope.

## 2. Overall Conclusion
- All listed issues from audit_report-1.md are fixed based on current static evidence.
- Final status: PASS (static fix-check).

## 3. Issue-by-Issue Fix Status

| Issue | Previous Severity | Status | Static Evidence | Fix-Check Notes |
|---|---|---|---|---|
| Self-service members could create orders for arbitrary patient IDs | Blocker | Fixed | repo/services/api/src/services/app_service/dining.rs:37, repo/services/api/src/services/app_service/dining.rs:41, repo/services/api/src/services/app_service/dining.rs:45, repo/API_tests/api_integration_tests.sh:501, repo/API_tests/api_integration_tests.sh:506 | Order creation now enforces patient access for non-global roles; tests now assign member to patient before self-service order creation. |
| Ticket split workflow lacked domain constraints and boundary validation | High | Fixed | repo/services/api/src/services/app_service/dining.rs:246, repo/services/api/src/services/app_service/dining.rs:248, repo/services/api/src/services/app_service/dining.rs:251, repo/services/web/src/pages/orders.rs:130 | Backend validates split_by enum and positive quantity; frontend uses constrained split type options. |
| API docs vs implementation mismatch for retention endpoint | Medium | Fixed | docs/api-spec.md:143, repo/services/api/src/routes/retention.rs:8 | Both now use /api/v1/retention/settings. |
| Missing-resource semantics diverged from documented 404 contract | Medium | Fixed | repo/services/api/src/services/app_service/clinical.rs:97, repo/services/api/src/services/app_service/dining.rs:45, repo/API_tests/api_integration_tests.sh:1049, repo/API_tests/api_integration_tests.sh:1064, repo/API_tests/api_integration_tests.sh:1067 | Access/missing handling now maps to NotFound paths in service logic; integration assertions now expect 404 for missing patient resource cases. |
| Core backend logic overly concentrated in oversized files | Medium | Fixed | repo/services/api/src/services/app_service/mod.rs:1, repo/services/api/src/services/app_service/dining.rs:1, repo/services/api/src/services/app_service/clinical.rs:1, repo/services/api/src/infrastructure/adapters/mysql_app_repository/mod.rs:1, repo/services/api/src/infrastructure/adapters/mysql_app_repository/ingestion.rs:1 | Monolith files were decomposed into domain modules under app_service and mysql_app_repository directories; previous monolith paths are removed. |
| Lockout duration documentation mismatch | Low | Fixed | repo/README.md:115, repo/services/api/config/default.toml:31 | README and config both specify 15-minute lockout. |

## 4. Architecture Delta Snapshot
- Refactor evidence:
  - Previous monolith service file removed: repo/services/api/src/services/app_service.rs (not present).
  - Previous monolith repository file removed: repo/services/api/src/infrastructure/adapters/mysql_app_repository.rs (not present).
  - Domain decomposition present under:
    - repo/services/api/src/services/app_service/
    - repo/services/api/src/infrastructure/adapters/mysql_app_repository/

## 5. Final Fix-Check Verdict
- Previous report issues resolved: 6/6
- Unresolved from previous report: 0/6
- Final verdict: PASS (static-only fix-check)
