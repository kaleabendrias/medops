#!/usr/bin/env bash
set -euo pipefail

REPORT_DIR="${1:-test_reports}"
mkdir -p "$REPORT_DIR"

start_ms=$(date +%s%3N)
docker compose run --rm --no-deps test_runner bash -lc "cd /workspace && /usr/local/cargo/bin/cargo test -p api-service --bin api-service"
end_ms=$(date +%s%3N)

cat >"$REPORT_DIR/backend_unit_tests.json" <<EOF
{"suite":"backend_unit_tests","status":"pass","duration_ms":$((end_ms - start_ms))}
EOF
