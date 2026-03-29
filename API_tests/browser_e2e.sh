#!/usr/bin/env bash
set -euo pipefail

REPORT_DIR="${1:-test_reports}"
mkdir -p "$REPORT_DIR"

CASE_FILE="$REPORT_DIR/browser_e2e.ndjson"
: >"$CASE_FILE"

record_case() {
  local name="$1"
  local status="$2"
  local detail="$3"
  printf '{"suite":"browser_e2e","case":"%s","status":"%s","detail":"%s"}\n' "$name" "$status" "$detail" >>"$CASE_FILE"
}

fail_case() {
  record_case "$1" "fail" "$2"
  cat >"$REPORT_DIR/browser_e2e.json" <<EOF
{"suite":"browser_e2e","status":"fail","failed_case":"$1"}
EOF
  exit 1
}

pass_case() {
  record_case "$1" "pass" "$2"
}

if ! command -v google-chrome >/dev/null 2>&1; then
  fail_case "browser_runtime_available" "google-chrome is required for browser-level E2E checks"
fi
pass_case "browser_runtime_available" "google-chrome detected"

cat >/tmp/browser-e2e-check.html <<'EOF'
<!doctype html>
<html>
<body>
  <pre id="out">RUNNING</pre>
  <script>
    (async () => {
      const out = document.getElementById('out')
      const base = 'http://localhost:8000/api/v1'

      async function login(username) {
        const res = await fetch(`${base}/auth/login`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ username, password: 'Admin#OfflinePass123' })
        })
        if (!res.ok) throw new Error(`login failed for ${username}: ${res.status}`)
        const payload = await res.json()
        return payload.token
      }

      try {
        const adminToken = await login('admin')
        const happy = await fetch(`${base}/patients/search?q=john`, {
          headers: { 'X-Session-Token': adminToken }
        })
        if (happy.status !== 200) {
          out.textContent = `FAIL happy-path expected 200 got ${happy.status}`
          return
        }

        const memberToken = await login('member1')
        const denied = await fetch(`${base}/patients/search?q=john`, {
          headers: { 'X-Session-Token': memberToken }
        })
        if (denied.status !== 403) {
          out.textContent = `FAIL denied-path expected 403 got ${denied.status}`
          return
        }

        out.textContent = 'PASS happy=200 denied=403'
      } catch (err) {
        out.textContent = `FAIL exception ${String(err)}`
      }
    })()
  </script>
</body>
</html>
EOF

python3 -m http.server 8099 --directory /tmp >/tmp/browser-e2e-http.log 2>&1 &
HTTP_PID=$!
cleanup_http() {
  kill "$HTTP_PID" >/dev/null 2>&1 || true
}
trap cleanup_http EXIT

dom=$(google-chrome --headless --disable-gpu --no-sandbox --disable-web-security --user-data-dir=/tmp/chrome-e2e --virtual-time-budget=12000 --dump-dom "http://127.0.0.1:8099/browser-e2e-check.html" 2>/tmp/browser-e2e.err || true)
printf '%s' "$dom" >/tmp/browser-e2e.dom

out_text=$(python3 - <<'PY'
import re
dom = open('/tmp/browser-e2e.dom', 'r', encoding='utf-8', errors='ignore').read()
m = re.search(r'<pre id="out">(.*?)</pre>', dom, re.S)
print((m.group(1).strip() if m else '').replace('\n', ' '))
PY
)

if [[ "$out_text" != PASS* ]]; then
  fail_case "browser_happy_and_denied_paths" "${out_text:-unable to read browser output}"
fi
pass_case "browser_happy_and_denied_paths" "$out_text"

cat >"$REPORT_DIR/browser_e2e.json" <<EOF
{"suite":"browser_e2e","status":"pass","cases":2}
EOF
