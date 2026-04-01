#!/usr/bin/env bash
set -euo pipefail

REPORT_DIR="${1:-test_reports}"
mkdir -p "$REPORT_DIR"

STEP_FILE="$REPORT_DIR/test_pipeline.ndjson"
: >"$STEP_FILE"

record_step() {
  local step="$1"
  local status="$2"
  local detail="$3"
  printf '{"step":"%s","status":"%s","detail":"%s"}\n' "$step" "$status" "$detail" >>"$STEP_FILE"
}

run_step() {
  local name="$1"
  shift
  set +e
  "$@"
  local rc=$?
  set -e
  if [ "$rc" -ne 0 ]; then
    record_step "$name" "fail" "exit_code=$rc"
    cat >"$REPORT_DIR/summary.json" <<EOF
{"status":"fail","failed_step":"$name"}
EOF
    exit "$rc"
  fi
  record_step "$name" "pass" "ok"
}

wait_for_api() {
  local max_attempts=60
  local attempt=1
  while [ "$attempt" -le "$max_attempts" ]; do
    if curl -fsS "http://localhost:8000/api/v1/health" >/dev/null 2>&1; then
      return 0
    fi
    sleep 2
    attempt=$((attempt + 1))
  done
  return 1
}

wait_for_web() {
  local max_attempts=60
  local attempt=1
  while [ "$attempt" -le "$max_attempts" ]; do
    if curl -fsS "http://localhost:8080/health" >/dev/null 2>&1; then
      return 0
    fi
    sleep 2
    attempt=$((attempt + 1))
  done
  return 1
}

run_step "compose_reset" docker compose down -v --remove-orphans
run_step "compose_build_up" docker compose up -d --build mysql api web
run_step "api_healthcheck" wait_for_api
run_step "web_healthcheck" wait_for_web

run_step "backend_unit_tests" bash unit_tests/run_backend_unit_tests.sh "$REPORT_DIR"
run_step "frontend_unit_tests" bash unit_tests/run_frontend_unit_tests.sh "$REPORT_DIR"
run_step "migration_checks" bash API_tests/migration_checks.sh "$REPORT_DIR"
run_step "authorization_matrix_checks" bash API_tests/authorization_matrix.sh "$REPORT_DIR"
run_step "api_integration_tests" bash API_tests/api_integration_tests.sh "$REPORT_DIR"
run_step "browser_e2e" bash API_tests/browser_e2e.sh "$REPORT_DIR"
run_step "e2e_smoke" bash API_tests/e2e_smoke.sh "$REPORT_DIR"

cat >"$REPORT_DIR/summary.json" <<EOF
{"status":"pass","report_dir":"$REPORT_DIR"}
EOF

echo "All tests passed. Reports written to $REPORT_DIR"
