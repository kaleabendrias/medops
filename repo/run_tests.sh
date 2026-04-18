#!/usr/bin/env bash
# All test execution runs inside Docker containers — no host-level curl,
# python3, or bash is required beyond the docker CLI itself.
set -euo pipefail

REPORT_DIR="${1:-test_reports}"
mkdir -p "$REPORT_DIR"

STEP_FILE="$REPORT_DIR/test_pipeline.ndjson"
: >"$STEP_FILE"

record_step() {
  local step="$1" status="$2" detail="$3"
  printf '{"step":"%s","status":"%s","detail":"%s"}\n' "$step" "$status" "$detail" >>"$STEP_FILE"
}

run_step() {
  local name="$1"; shift
  set +e; "$@"; local rc=$?; set -e
  if [ "$rc" -ne 0 ]; then
    record_step "$name" "fail" "exit_code=$rc"
    cat >"$REPORT_DIR/summary.json" <<EOF
{"status":"fail","failed_step":"$name"}
EOF
    exit "$rc"
  fi
  record_step "$name" "pass" "ok"
}

# Wait for a container to reach the "healthy" state using docker inspect only —
# no host-side curl or wget required.
wait_for_healthy() {
  local container="$1"
  local max=60 attempt=1
  until [ "$(docker inspect --format='{{.State.Health.Status}}' "$container" 2>/dev/null)" = "healthy" ]; do
    [ "$attempt" -ge "$max" ] && return 1
    attempt=$((attempt + 1))
    sleep 2
  done
}

# Bring up the application stack and build the integration test runner image.
run_step "compose_reset"    docker compose down -v --remove-orphans
run_step "compose_build_up" docker compose up -d --build mysql api web
run_step "integration_tests_build" docker compose build integration_tests
run_step "playwright_tests_build"  docker compose build playwright_tests

# Wait for each service using Docker healthcheck status (no host curl).
run_step "api_healthcheck" wait_for_healthy "monorepo_api"
run_step "web_healthcheck" wait_for_healthy "monorepo_web"

# Unit tests run inside the test_runner container (no network dependency).
run_step "backend_unit_tests"  bash unit_tests/run_backend_unit_tests.sh  "$REPORT_DIR"
run_step "frontend_unit_tests" bash unit_tests/run_frontend_unit_tests.sh "$REPORT_DIR"

# DB integration tests run against a dedicated test_db container to avoid
# polluting the application database with test data.
run_step "test_db_start"         docker compose --profile testing up -d --force-recreate test_db
run_step "test_db_healthcheck"   wait_for_healthy "monorepo_test_db"
run_step "db_integration_tests"  docker compose --profile testing run --rm -T db_test_runner \
  bash unit_tests/run_db_integration_tests.sh "$REPORT_DIR"

# All integration/E2E suites execute entirely inside the integration_tests
# container which reaches api:8000, web:8443 (HTTPS), and mysql:3306 over the
# internal Docker network — no host-side curl, python3, or mysql client needed.
run_integration() {
  local step="$1" script="$2"
  run_step "$step" docker compose run --rm -T integration_tests bash "$script" test_reports
}

run_integration "migration_checks"            "API_tests/migration_checks.sh"
run_integration "authorization_matrix_checks" "API_tests/authorization_matrix.sh"
run_integration "api_integration_tests"       "API_tests/api_integration_tests.sh"
run_integration "e2e_smoke"                   "API_tests/e2e_smoke.sh"
run_integration "browser_e2e"                 "API_tests/browser_e2e.sh"

# Playwright browser E2E — real DOM interactions via Chromium against web:8443.
run_step "playwright_e2e" docker compose run --rm -T playwright_tests \
  bash API_tests/playwright_e2e.sh test_reports

cat >"$REPORT_DIR/summary.json" <<EOF
{"status":"pass","report_dir":"$REPORT_DIR"}
EOF
echo "All tests passed. Reports written to $REPORT_DIR"
