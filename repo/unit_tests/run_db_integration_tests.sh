#!/usr/bin/env bash
# Runs the true no-mock integration tests against the test_db MySQL container.
# DATABASE_TEST_URL must point at a live hospital_test database; the db_test_runner
# Docker Compose service sets this automatically.
set -euo pipefail

REPORT_DIR="${1:-test_reports}"
mkdir -p "$REPORT_DIR"

start_ms=$(date +%s%3N)
/usr/local/cargo/bin/cargo test -p api-service --test repository_integration
end_ms=$(date +%s%3N)

cat >"$REPORT_DIR/db_integration_tests.json" <<EOF
{"suite":"db_integration_tests","status":"pass","duration_ms":$((end_ms - start_ms))}
EOF
