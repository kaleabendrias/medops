# API_tests

Container-backed API verification suites:

- `migration_checks.sh` validates migration application and seeded deterministic fixtures.
- `authorization_matrix.sh` validates RBAC allow/deny paths.
- `api_integration_tests.sh` validates policy, lifecycle, and governance behavior.
- `browser_e2e.sh` validates browser-executed happy-path and permission-denied flows.
- `e2e_smoke.sh` validates integrated role journeys and API-side workflow guards.

Each script writes NDJSON case logs and a JSON suite summary.
