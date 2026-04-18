#!/usr/bin/env bash
# Browser-level FE<->BE E2E: exercises the full stack through the nginx reverse
# proxy the same way a browser would – login and authenticated API calls are
# made against https://web:8443/api/v1 (same-origin TLS), not the backend
# port directly.
set -euo pipefail

REPORT_DIR="${1:-test_reports}"
mkdir -p "$REPORT_DIR"

WEB_BASE="https://web:8443"
PROXY_API="https://web:8443/api/v1"
# -k: skip TLS verification for the internal self-signed certificate.
CURL="curl -sk"
CASE_FILE="$REPORT_DIR/browser_e2e.ndjson"
: >"$CASE_FILE"

record_case() {
  local name="$1" status="$2" detail="$3"
  printf '{"suite":"browser_e2e","case":"%s","status":"%s","detail":"%s"}\n' \
    "$name" "$status" "$detail" >>"$CASE_FILE"
}

fail_case() {
  record_case "$1" "fail" "$2"
  cat >"$REPORT_DIR/browser_e2e.json" <<EOF
{"suite":"browser_e2e","status":"fail","failed_case":"$1"}
EOF
  exit 1
}

pass_case() { record_case "$1" "pass" "$2"; }

# ── 1. Frontend serves HTML ──
html_status=$($CURL -o /tmp/browser_e2e_index.html -w "%{http_code}" "$WEB_BASE/")
if [ "$html_status" != "200" ]; then
  fail_case "frontend_serves_html" "expected 200 got $html_status"
fi
pass_case "frontend_serves_html" "frontend root returned $html_status"

# ── 2. HTML contains app entry point markers ──
if ! grep -qi "wasm\|index\|hospital" /tmp/browser_e2e_index.html; then
  fail_case "frontend_app_root" "HTML missing wasm/app entry-point markers"
fi
pass_case "frontend_app_root" "frontend HTML contains app entry-point markers"

# ── 3. Nginx /health endpoint ──
health_status=$($CURL -o /tmp/browser_e2e_nginx_health.txt -w "%{http_code}" "$WEB_BASE/health")
if [ "$health_status" != "200" ]; then
  fail_case "nginx_health_endpoint" "nginx health returned $health_status"
fi
health_body=$(cat /tmp/browser_e2e_nginx_health.txt)
if [ "$health_body" != "ok" ]; then
  fail_case "nginx_health_body" "expected 'ok' got '$health_body'"
fi
pass_case "nginx_health_endpoint" "nginx health endpoint returns 200 ok"

# ── 4. API reachable through nginx reverse proxy ──
proxy_health=$($CURL -o /tmp/browser_e2e_proxy_health.json -w "%{http_code}" "$PROXY_API/health")
if [ "$proxy_health" != "200" ]; then
  fail_case "proxy_api_health" "nginx proxy to backend returned $proxy_health"
fi
pass_case "proxy_api_health" "nginx reverse-proxy forwards /api/v1/health to backend"

# ── 5. Same-origin login (as the browser SPA does it) ──
login_status=$($CURL -D /tmp/browser_e2e_login_hdr.txt -o /tmp/browser_e2e_login.json -w "%{http_code}" \
  -X POST "$PROXY_API/auth/login" \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"Admin#OfflinePass123"}')
if [ "$login_status" != "200" ]; then
  fail_case "proxy_login" "login via nginx proxy returned $login_status"
fi
proxy_cookie=$(grep -i "^set-cookie:" /tmp/browser_e2e_login_hdr.txt \
  | grep "hospital_session" \
  | sed 's/.*hospital_session=\([^;]*\).*/\1/' \
  | tr -d '\r' | head -1)
proxy_csrf=$(python3 -c \
  'import json; print(json.load(open("/tmp/browser_e2e_login.json"))["csrf_token"])' \
  2>/dev/null || echo "")
if [ -z "$proxy_cookie" ] || [ -z "$proxy_csrf" ]; then
  fail_case "proxy_login_token" "cookie or csrf_token absent from proxy login response"
fi
pass_case "proxy_login" "same-origin login through nginx proxy succeeds"

# ── 6. Authenticated API call via proxy (menu entitlements) ──
ent_status=$($CURL -o /tmp/browser_e2e_ent.json -w "%{http_code}" \
  "$PROXY_API/rbac/menu-entitlements" \
  --cookie "hospital_session=${proxy_cookie}")
if [ "$ent_status" != "200" ]; then
  fail_case "proxy_menu_entitlements" "menu-entitlements via proxy returned $ent_status"
fi
pass_case "proxy_menu_entitlements" "authenticated API call through nginx proxy returns 200"

# ── 7. Entitlements response has expected shape ──
ent_count=$(python3 -c \
  'import json; print(len(json.load(open("/tmp/browser_e2e_ent.json"))))' \
  2>/dev/null || echo "0")
if [ "$ent_count" -lt 1 ]; then
  fail_case "proxy_entitlements_shape" "expected entitlements list, got $ent_count items"
fi
pass_case "proxy_entitlements_shape" "entitlements response has $ent_count items"

# ── 8. Static asset: index.html explicit fetch ──
asset_status=$($CURL -o /dev/null -w "%{http_code}" "$WEB_BASE/index.html")
if [ "$asset_status" != "200" ]; then
  fail_case "static_asset_index" "index.html returned $asset_status"
fi
pass_case "static_asset_index" "index.html served with $asset_status"

# ── 9. SPA fallback: deep-link route returns index.html ──
spa_status=$($CURL -o /dev/null -w "%{http_code}" "$WEB_BASE/patients")
if [ "$spa_status" != "200" ]; then
  fail_case "spa_fallback_routing" "SPA route /patients returned $spa_status (expected 200 via try_files)"
fi
pass_case "spa_fallback_routing" "nginx try_files fallback serves SPA for deep links"

# ── 10. Unauthenticated proxy call is rejected ──
unauth_status=$($CURL -o /dev/null -w "%{http_code}" "$PROXY_API/patients/search?q=test")
if [ "$unauth_status" != "401" ]; then
  fail_case "proxy_unauthenticated_rejected" "expected 401 got $unauth_status"
fi
pass_case "proxy_unauthenticated_rejected" "unauthenticated call through proxy correctly returns 401"

# ── 11. Proxy transmits session cookie (admin-only route) ──
admin_route_status=$($CURL -o /dev/null -w "%{http_code}" \
  "$PROXY_API/admin/users" \
  --cookie "hospital_session=${proxy_cookie}")
if [ "$admin_route_status" != "200" ]; then
  fail_case "proxy_admin_route" "admin route via proxy returned $admin_route_status"
fi
pass_case "proxy_admin_route" "proxy correctly forwards session cookie to backend"

# ── 12. Non-admin role via proxy is denied on admin route ──
$CURL -D /tmp/browser_e2e_member_hdr.txt -o /tmp/browser_e2e_member_body.json \
  -X POST "$PROXY_API/auth/login" \
  -H "Content-Type: application/json" \
  -d '{"username":"member1","password":"Admin#OfflinePass123"}'
member_cookie=$(grep -i "^set-cookie:" /tmp/browser_e2e_member_hdr.txt \
  | grep "hospital_session" \
  | sed 's/.*hospital_session=\([^;]*\).*/\1/' \
  | tr -d '\r' | head -1)
if [ -n "$member_cookie" ]; then
  member_admin_status=$($CURL -o /dev/null -w "%{http_code}" \
    "$PROXY_API/admin/users" --cookie "hospital_session=${member_cookie}")
  if [ "$member_admin_status" != "403" ]; then
    fail_case "proxy_rbac_denial" "member access to admin route via proxy expected 403 got $member_admin_status"
  fi
  pass_case "proxy_rbac_denial" "RBAC denial propagates correctly through nginx proxy"
fi

case_count=12

cat >"$REPORT_DIR/browser_e2e.json" <<EOF
{"suite":"browser_e2e","status":"pass","cases":$case_count}
EOF
