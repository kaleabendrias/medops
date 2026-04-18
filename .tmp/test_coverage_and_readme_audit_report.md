# Test Coverage Audit

## Scope and Mode
- Audit mode: static inspection only (no execution performed).
- Target repo: `repo/`.
- Project type declaration at README top: `fullstack` (explicitly present in `repo/README.md`).
- Endpoint definition used: unique `METHOD + normalized PATH` where dynamic segments are normalized to `:param`.

## Backend Endpoint Inventory (72)
1. GET /api/v1/health
2. POST /api/v1/auth/login
3. POST /api/v1/auth/logout
4. GET /api/v1/hospitals
5. GET /api/v1/roles
6. GET /api/v1/rbac/menu-entitlements
7. GET /api/v1/admin/users
8. POST /api/v1/admin/users/:param/disable
9. POST /api/v1/patients
10. GET /api/v1/patients
11. GET /api/v1/patients/search
12. GET /api/v1/patients/:param
13. POST /api/v1/patients/:param/assign
14. PUT /api/v1/patients/:param
15. PUT /api/v1/patients/:param/allergies
16. PUT /api/v1/patients/:param/contraindications
17. PUT /api/v1/patients/:param/history
18. POST /api/v1/patients/:param/visit-notes
19. GET /api/v1/patients/:param/revisions
20. POST /api/v1/patients/:param/attachments
21. GET /api/v1/patients/:param/attachments
22. GET /api/v1/patients/:param/attachments/:param/download
23. GET /api/v1/patients/:param/export
24. GET /api/v1/bedboard/beds
25. POST /api/v1/bedboard/beds/:param/transition
26. GET /api/v1/bedboard/events
27. POST /api/v1/dining/menus
28. GET /api/v1/dining/menus
29. POST /api/v1/orders
30. PUT /api/v1/orders/:param/status
31. GET /api/v1/orders
32. POST /api/v1/orders/:param/ticket-splits
33. GET /api/v1/orders/:param/ticket-splits
34. POST /api/v1/orders/:param/notes
35. GET /api/v1/orders/:param/notes
36. GET /api/v1/cafeteria/categories
37. POST /api/v1/cafeteria/dishes
38. GET /api/v1/cafeteria/dishes
39. PUT /api/v1/cafeteria/dishes/:param/status
40. POST /api/v1/cafeteria/dishes/:param/options
41. POST /api/v1/cafeteria/dishes/:param/windows
42. PUT /api/v1/cafeteria/ranking-rules
43. GET /api/v1/cafeteria/ranking-rules
44. GET /api/v1/cafeteria/recommendations
45. POST /api/v1/campaigns
46. POST /api/v1/campaigns/:param/join
47. GET /api/v1/campaigns
48. POST /api/v1/experiments
49. POST /api/v1/experiments/:param/variants
50. POST /api/v1/experiments/:param/assign
51. POST /api/v1/experiments/:param/backtrack
52. GET /api/v1/analytics/funnel
53. GET /api/v1/analytics/retention
54. GET /api/v1/analytics/recommendation-kpi
55. POST /api/v1/governance/records
56. GET /api/v1/governance/records
57. DELETE /api/v1/governance/records/:param
58. POST /api/v1/ingestion/tasks
59. PUT /api/v1/ingestion/tasks/:param
60. POST /api/v1/ingestion/tasks/:param/rollback
61. POST /api/v1/ingestion/tasks/:param/run
62. GET /api/v1/ingestion/tasks
63. GET /api/v1/ingestion/tasks/:param/versions
64. GET /api/v1/ingestion/tasks/:param/runs
65. POST /api/v1/telemetry/events
66. GET /api/v1/audits
67. PUT /api/v1/audits
68. DELETE /api/v1/audits
69. GET /api/v1/retention/settings
70. GET /api/v1/retention/policies
71. PUT /api/v1/retention/policies/:param/:param
72. GET /api/v1/session

Endpoint source evidence: route macros in `repo/services/api/src/main.rs` and `repo/services/api/src/routes/*.rs` (e.g., `#[rocket::get(...)]`, `#[rocket::post(...)]`).

## API Test Mapping Table

| Endpoint | Covered | Test Type | Test Files | Evidence |
|---|---|---|---|---|
| GET /api/v1/health | yes | true no-mock HTTP | `repo/API_tests/browser_e2e.sh` | case `proxy_api_health` uses curl to `$PROXY_API/health` |
| POST /api/v1/auth/login | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh`, `repo/API_tests/authorization_matrix.sh`, `repo/API_tests/e2e_smoke.sh`, `repo/API_tests/browser_e2e.sh` | login helpers `login_user`, case `proxy_login` |
| POST /api/v1/auth/logout | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | case `logout_returns_200` |
| GET /api/v1/hospitals | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh`, `repo/API_tests/authorization_matrix.sh`, `repo/API_tests/e2e_smoke.sh` | cases `catalog_hospitals_admin_allowed`, `catalog_requires_auth` |
| GET /api/v1/roles | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | case `catalog_roles_list_integration` |
| GET /api/v1/rbac/menu-entitlements | yes | true no-mock HTTP | `repo/API_tests/authorization_matrix.sh`, `repo/API_tests/e2e_smoke.sh`, `repo/API_tests/browser_e2e.sh` | function `menu_allowed`, case `admin_journey_entitlements`, case `proxy_menu_entitlements` |
| GET /api/v1/admin/users | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh`, `repo/API_tests/authorization_matrix.sh`, `repo/API_tests/browser_e2e.sh` | cases `admin_users_allow`, `admin_users_allow_admin`, `proxy_admin_route` |
| POST /api/v1/admin/users/:param/disable | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | case `admin_disable_user` |
| POST /api/v1/patients | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh`, `repo/API_tests/authorization_matrix.sh` | cases `create_patient_for_scenarios`, patient-create fallback in auth matrix |
| GET /api/v1/patients | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | case `patients_list` |
| GET /api/v1/patients/search | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh`, `repo/API_tests/authorization_matrix.sh`, `repo/API_tests/e2e_smoke.sh`, Playwright E2E | cases `rbac_allow_admin_patient_search`, `patients_search_allow_clinical`, `clinical_journey_patients` |
| GET /api/v1/patients/:param | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | cases `patient_object_access_after_assignment`, `patient_nonexistent_denied` |
| POST /api/v1/patients/:param/assign | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | cases `patient_assignment_create`, `member_patient_assignment_for_self_service` |
| PUT /api/v1/patients/:param | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | cases `patient_revision_delta_demographics_update`, `patient_revision_reason_required` |
| PUT /api/v1/patients/:param/allergies | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | case `patient_revision_delta_clinical_update` |
| PUT /api/v1/patients/:param/contraindications | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | case `patient_contraindications_update` |
| PUT /api/v1/patients/:param/history | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | case `patient_history_update` |
| POST /api/v1/patients/:param/visit-notes | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | case `patient_visit_note_add` |
| GET /api/v1/patients/:param/revisions | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | cases `patient_revision_delta_list_masked`, `patient_revisions_nonexistent_denied` |
| POST /api/v1/patients/:param/attachments | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | cases `attachment_type_constraint`, `attachment_binary_upload` |
| GET /api/v1/patients/:param/attachments | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | case `attachment_binary_metadata` |
| GET /api/v1/patients/:param/attachments/:param/download | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | cases `attachment_binary_download`, `attachment_nonexistent_404` |
| GET /api/v1/patients/:param/export | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh`, `repo/API_tests/authorization_matrix.sh`, `repo/API_tests/e2e_smoke.sh` | cases `patient_export_json`, `patient_export_allow_clinical`, `clinical_journey_export` |
| GET /api/v1/bedboard/beds | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | case uses `beds_json=$(curl ... /bedboard/beds)` and `invalid_token_bedboard_rejected` |
| POST /api/v1/bedboard/beds/:param/transition | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | cases `bed_state_machine_legal_transition`, `bed_state_machine_illegal_transition` |
| GET /api/v1/bedboard/events | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | case `bedboard_events_list` |
| POST /api/v1/dining/menus | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | cases `campaign_menu_create`, `menu_create` |
| GET /api/v1/dining/menus | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | `campaign_menus_json=$(curl ... /dining/menus)` |
| POST /api/v1/orders | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh`, `repo/API_tests/authorization_matrix.sh` | cases `order_create`, `member_self_service_order_create`, auth matrix admin order creation |
| PUT /api/v1/orders/:param/status | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh`, `repo/API_tests/authorization_matrix.sh` | cases `order_status_billed`, `order_status_nonexistent_404`, auth matrix `member_cross_order_mutate_denied` |
| GET /api/v1/orders | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh`, `repo/API_tests/authorization_matrix.sh`, `repo/API_tests/e2e_smoke.sh` | cases `member_list_orders_scoped`, `orders_require_session`, `member_journey_orders` |
| POST /api/v1/orders/:param/ticket-splits | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh`, `repo/API_tests/authorization_matrix.sh` | cases `order_ticket_split_add`, `member_cross_order_split_denied` |
| GET /api/v1/orders/:param/ticket-splits | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh`, `repo/API_tests/authorization_matrix.sh` | cases `order_ticket_split_list`, `member_cross_order_read_splits_forbidden` |
| POST /api/v1/orders/:param/notes | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh`, `repo/API_tests/authorization_matrix.sh` | cases `order_note_add`, `member_cross_order_note_denied` |
| GET /api/v1/orders/:param/notes | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh`, `repo/API_tests/authorization_matrix.sh` | cases `order_note_list`, `member_cross_order_read_denied` |
| GET /api/v1/cafeteria/categories | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh`, `repo/API_tests/authorization_matrix.sh`, `repo/API_tests/e2e_smoke.sh`, Playwright E2E | case `cafeteria_categories_list`, `cafeteria_allow_inventory_read`, `cafeteria_journey_inventory` |
| POST /api/v1/cafeteria/dishes | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh`, `repo/API_tests/authorization_matrix.sh`, `repo/API_tests/e2e_smoke.sh` | cases `campaign_dish_create`, `clinical_deny_inventory_write`, `clinical_journey_dining_management_denied` |
| GET /api/v1/cafeteria/dishes | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | case `cafeteria_dishes_list` |
| PUT /api/v1/cafeteria/dishes/:param/status | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | cases `campaign_dish_publish`, `test_meal_dish_publish` |
| POST /api/v1/cafeteria/dishes/:param/options | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | case `cafeteria_dish_option_add` |
| POST /api/v1/cafeteria/dishes/:param/windows | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | cases `campaign_dish_sales_window`, `test_meal_dish_sales_window` |
| PUT /api/v1/cafeteria/ranking-rules | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | case `cafeteria_ranking_rule_upsert` |
| GET /api/v1/cafeteria/ranking-rules | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | case `cafeteria_ranking_rules_list` |
| GET /api/v1/cafeteria/recommendations | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | case `cafeteria_recommendations_list` |
| POST /api/v1/campaigns | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | cases `campaign_create_with_deadline`, `campaign_create_for_deadline` |
| POST /api/v1/campaigns/:param/join | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | cases `campaign_join_admin`, `campaign_join_member` |
| GET /api/v1/campaigns | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | cases `campaign_success_refresh`, `campaign_list_after_deadline` |
| POST /api/v1/experiments | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | cases `experiment_metrics_suite_create`, `experiment_create_lifecycle` |
| POST /api/v1/experiments/:param/variants | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | cases `experiment_variant_add_control`, `experiment_variant_add_treatment` |
| POST /api/v1/experiments/:param/assign | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | case `experiment_variant_assign` |
| POST /api/v1/experiments/:param/backtrack | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | case `experiment_variant_backtrack` |
| GET /api/v1/analytics/funnel | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh`, `repo/API_tests/authorization_matrix.sh` | cases `analytics_funnel_available`, `analytics_allow_admin` |
| GET /api/v1/analytics/retention | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | case `analytics_retention_cohorts` |
| GET /api/v1/analytics/recommendation-kpi | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | case `analytics_recommendation_kpi` |
| POST /api/v1/governance/records | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | cases `governance_create_raw`, `governance_create_analytics` |
| GET /api/v1/governance/records | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh`, `repo/API_tests/authorization_matrix.sh` | cases `governance_list_records`, `governance_allow_admin` |
| DELETE /api/v1/governance/records/:param | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | case `governance_tombstone_request` |
| POST /api/v1/ingestion/tasks | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh`, `repo/API_tests/authorization_matrix.sh` | cases `ingestion_create_task`, `ingestion_create_allow_admin` |
| PUT /api/v1/ingestion/tasks/:param | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | case `ingestion_update_task` |
| POST /api/v1/ingestion/tasks/:param/rollback | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | case `ingestion_rollback_task` |
| POST /api/v1/ingestion/tasks/:param/run | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh`, `repo/API_tests/authorization_matrix.sh` | cases `ingestion_run_task`, `ingestion_run_deny_cafeteria` |
| GET /api/v1/ingestion/tasks | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh`, `repo/API_tests/authorization_matrix.sh`, `repo/API_tests/e2e_smoke.sh` | cases `ingestion_list_tasks`, `ingestion_allow_admin`, `admin_journey_ingestion` |
| GET /api/v1/ingestion/tasks/:param/versions | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | case `ingestion_versions_after_update` |
| GET /api/v1/ingestion/tasks/:param/runs | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | cases `ingestion_runs_list`, `ingestion_deterministic_failure_run_list` |
| POST /api/v1/telemetry/events | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | cases `audit_append_only_trigger_action`, `telemetry_order_created_ingest` |
| GET /api/v1/audits | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh`, `repo/API_tests/authorization_matrix.sh` | cases `audits_list_allow_admin`, `audits_allow_admin` |
| PUT /api/v1/audits | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | case `audit_api_update_rejected` |
| DELETE /api/v1/audits | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | case `audit_api_delete_rejected` |
| GET /api/v1/retention/settings | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | case `retention_settings_accessible` |
| GET /api/v1/retention/policies | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh`, `repo/API_tests/authorization_matrix.sh` | cases `retention_policies_list`, `retention_allow_admin` |
| PUT /api/v1/retention/policies/:param/:param | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | cases `retention_policy_upsert`, `retention_policy_floor_enforced` |
| GET /api/v1/session | yes | true no-mock HTTP | `repo/API_tests/api_integration_tests.sh` | cases `session_active_at_479_minutes`, `session_expired_at_481_minutes` |

Additional mocked HTTP evidence exists for subset routes in `repo/services/api/src/routes/tests.rs` (`Client::tracked` + `StubRepo`), e.g. tests `governance_list_authenticated_with_permission_returns_200`, `ingestion_list_authenticated_with_permission_returns_200`.

## API Test Classification

### 1. True No-Mock HTTP
- `repo/API_tests/api_integration_tests.sh`
- `repo/API_tests/authorization_matrix.sh`
- `repo/API_tests/e2e_smoke.sh`
- `repo/API_tests/browser_e2e.sh`
- `repo/API_tests/playwright_e2e.sh` + `repo/API_tests/playwright/tests/*.spec.js`

Rationale: each uses real HTTP requests to running services (API directly via `http://api:8000/api/v1` or via nginx proxy `https://web:8443/api/v1`), with no test-level service/controller mocking in the request execution path.

### 2. HTTP with Mocking
- `repo/services/api/src/routes/tests.rs`

Rationale: in-process Rocket client is used, but backing repository is `StubRepo` implementing `AppRepository`; database/business dependencies are stubbed.

### 3. Non-HTTP (unit/integration without HTTP)
- `repo/services/api/src/services/app_service/tests.rs`
- `repo/services/api/src/services/app_service/auth.rs` (internal tests)
- `repo/services/api/tests/repository_integration.rs`
- `repo/services/api/src/infrastructure/*` module tests (e.g., logging, middleware, crypto, repository helper tests)

## Mock Detection
- WHAT: repository/service dependency replacement via `StubRepo`.
- WHERE:
  - `repo/services/api/src/routes/tests.rs` (type `StubRepo`, trait impl `impl AppRepository for StubRepo`, test client builders `build_client`, `build_client_full`).
- Classification impact: these are `HTTP test with mocking`, not true no-mock API tests.

No `jest.mock`, `vi.mock`, or `sinon.stub` usage was found in inspected API test files.

## Coverage Summary
- Total endpoints: 72
- Endpoints with HTTP tests (any HTTP type): 72
- Endpoints with TRUE no-mock HTTP tests: 72

Computed metrics:
- HTTP coverage % = 72 / 72 = 100%
- True API coverage % = 72 / 72 = 100%

## Unit Test Summary

### Backend Unit Tests
- Test files / clusters found:
  - `repo/services/api/src/services/app_service/tests.rs`
  - `repo/services/api/src/services/app_service/auth.rs` (internal tests)
  - `repo/services/api/src/routes/tests.rs` (controller route behavior with stubbed repository)
  - `repo/services/api/src/infrastructure/auth/middleware.rs`
  - `repo/services/api/src/infrastructure/logging.rs`
  - `repo/services/api/src/infrastructure/security/field_crypto.rs`
  - `repo/services/api/src/infrastructure/adapters/mysql_app_repository/*.rs` (module-level tests)
  - `repo/services/api/tests/repository_integration.rs` (real MySQL integration, non-HTTP)
- Modules covered:
  - Controllers/routes: covered (`routes/tests.rs`) and no-mock HTTP via shell suites.
  - Services: covered (`app_service/tests.rs`, `app_service/auth.rs`).
  - Repositories: covered (mysql repository module tests + `repository_integration.rs`).
  - Auth/guards/middleware: covered (`infrastructure/auth/middleware.rs`, auth/login flows in API tests).
- Important backend modules NOT clearly unit-tested in-file (based on direct test annotations in inspected files):
  - `repo/services/api/src/config.rs`
  - `repo/services/api/src/infrastructure/database.rs`
  - `repo/services/api/src/repositories/app_repository.rs` (trait definition, no direct tests expected but listed for completeness)

### Frontend Unit Tests (STRICT REQUIREMENT)
Detection checks:
- identifiable frontend test files exist: YES (`repo/services/web/src/ui_logic.test.rs`, `repo/services/web/src/state.test.rs`)
- tests target frontend logic/components: YES (state/navigation/ui logic/component helpers in `repo/services/web/src/*`)
- test framework evident: YES (Rust `#[test]` executed via `cargo test -p web-app` in `repo/unit_tests/run_frontend_unit_tests.sh`)
- tests import actual frontend modules/components: YES (`use crate::ui_logic::...`, `use crate::state::...`, plus many in-module tests)

Frontend test files (explicit `.test.*`):
- `repo/services/web/src/ui_logic.test.rs`
- `repo/services/web/src/state.test.rs`

Additional frontend unit tests embedded in modules include:
- `repo/services/web/src/components/app_shell.rs`
- `repo/services/web/src/components/auth_gate.rs`
- `repo/services/web/src/components/feedback.rs`
- `repo/services/web/src/features/*.rs`
- `repo/services/web/src/hooks/orders.rs`
- `repo/services/web/src/api/mod.rs`
- `repo/services/web/src/ui_logic.rs`
- `repo/services/web/src/state.rs`

Framework/tools detected:
- Rust built-in test harness (`#[test]`, cargo test)
- Playwright for browser E2E (`repo/API_tests/playwright/package.json`, `@playwright/test`)

Important frontend components/modules NOT explicitly unit-tested as page modules (no direct page-level tests observed for many pages):
- `repo/services/web/src/pages/patients.rs`
- `repo/services/web/src/pages/orders.rs`
- `repo/services/web/src/pages/dining.rs`
- `repo/services/web/src/pages/campaigns.rs`
- `repo/services/web/src/pages/experiments.rs`
- `repo/services/web/src/pages/analytics.rs`
- `repo/services/web/src/pages/bedboard.rs`
- `repo/services/web/src/pages/ingestion.rs`
- `repo/services/web/src/pages/audits.rs`
- `repo/services/web/src/pages/dashboard.rs`

Mandatory verdict:
- Frontend unit tests: PRESENT

Strict failure rule result:
- No CRITICAL GAP triggered for missing frontend unit tests (tests are present by file-level evidence).

### Cross-Layer Observation
- Backend testing is substantially heavier (broad no-mock API matrix + extensive service/repository tests).
- Frontend testing exists and is non-trivial, but depth is concentrated in logic/state/helpers rather than page-component behavior.
- Overall balance: acceptable for fullstack, but backend remains dominant.

## API Observability Check
- Strong observability suites:
  - `repo/API_tests/api_integration_tests.sh` (explicit method+path, request payloads, schema/content assertions)
  - `repo/API_tests/authorization_matrix.sh` (explicit role-path matrix and response checks)
- Weaker observability suites:
  - `repo/API_tests/e2e_smoke.sh` (primarily status checks; limited response-content depth)
  - `repo/API_tests/playwright/tests/*.spec.js` (UI-focused; endpoint method/path often inferred from response URL substrings)

## Tests Check
- `repo/run_tests.sh` is Docker-based and containerized end-to-end.
- No host-side package manager install steps are required to run the standard pipeline.
- Result against rule: Docker-based -> OK.

## End-to-End Expectations (Fullstack)
- Real FE ↔ BE tests exist:
  - proxy/transport E2E via curl: `repo/API_tests/browser_e2e.sh`
  - browser DOM E2E via Playwright: `repo/API_tests/playwright/tests/*.spec.js`
- Requirement satisfied.

## Test Quality & Sufficiency
- Strengths:
  - Full endpoint matrix coverage with real HTTP requests.
  - Strong auth/RBAC/object-isolation/security regression checks.
  - Good negative/edge scenarios (invalid token, non-existent IDs, lockout, version conflict, CSRF, retention floor, audit immutability).
  - Meaningful payload and response schema assertions in core suites.
- Weaknesses:
  - A subset of route tests relies on `StubRepo` (mocked dependency path).
  - Some smoke/UI suites emphasize status/visibility over deep payload semantics.

## Test Coverage Score (0–100)
- Score: 92

## Score Rationale
- + Endpoint coverage and true HTTP coverage are both complete (100%).
- + No-mock HTTP validation is broad and realistic.
- + Security/RBAC/negative-path depth is strong.
- - Some controller HTTP tests are mock-backed (`StubRepo`), reducing end-to-end confidence for that subset.
- - Frontend unit emphasis is more logic-focused than full page behavior.

## Key Gaps
1. Mock-backed controller route tests (`repo/services/api/src/routes/tests.rs`) do not validate repository/database behavior.
2. Several frontend page modules lack direct unit-level tests (reliance shifts to E2E).
3. Observability depth is uneven: smoke/UI tests are less explicit on request/response contracts than integration suites.

## Confidence & Assumptions
- Confidence: high.
- Assumptions:
  - Endpoint inventory derived from route macros in `main.rs` + `routes/*.rs` is complete.
  - Static analysis cannot prove runtime reachability under all startup conditions.
  - Classification is based on visible test code only.

---

# README Audit

## Target File
- Required README location exists: `repo/README.md`.

## Project Type Detection
- Declared at top: `fullstack` (line 1 of README content).
- Inference fallback not needed.

## Hard Gates

### Formatting
- PASS: markdown is structured and readable (clear sections/tables/code blocks).

### Startup Instructions (backend/fullstack)
- PASS: includes `docker-compose up` under "Running the Application".

### Access Method
- PASS: explicit URL/port guidance present:
  - frontend `https://localhost:8443`
  - proxied API `https://localhost:8443/api/v1`
  - internal API/DB service addresses documented.

### Verification Method
- PASS: includes explicit verification flows:
  - End-user workflow checklist
  - Manual API verification with curl for auth/search/audits and expected response shapes.

### Environment Rules (Docker-contained)
- PASS: README states zero-config Docker workflow and avoids instructing host runtime/package installs for app startup.
- Note: `chmod +x run_tests.sh` appears, but this is not a runtime dependency install.

### Demo Credentials (auth present)
- PASS: credentials provided with roles and passwords for Admin, Member, Employee, Clinical, Cafeteria, Locked user.

## Engineering Quality
- Tech stack clarity: strong.
- Architecture explanation: strong (request flow, container boundaries).
- Testing instructions: strong (suite breakdown + single entrypoint + individual suites).
- Security/roles: strong (RBAC, CSRF, cookie/session, encryption, audit append-only).
- Workflow guidance: strong.
- Presentation quality: strong.

## High Priority Issues
- None.

## Medium Priority Issues
1. README claims no host `curl` needed in prerequisites, while manual verification section demonstrates host curl commands; this is not a hard-gate failure but is a messaging inconsistency.

## Low Priority Issues
1. Mixed use of `docker-compose` and `docker compose` style commands could be normalized for consistency.

## Hard Gate Failures
- None.

## README Verdict
- PASS

---

# Final Verdicts
- Test Coverage Audit Verdict: PASS (with non-critical quality gaps).
- README Audit Verdict: PASS.
