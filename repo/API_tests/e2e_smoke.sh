#!/usr/bin/env bash
set -euo pipefail

REPORT_DIR="${1:-test_reports}"
mkdir -p "$REPORT_DIR"

API_BASE="http://localhost:8000/api/v1"
CASE_FILE="$REPORT_DIR/e2e_smoke.ndjson"
: >"$CASE_FILE"

record_case() {
  local name="$1"
  local status="$2"
  local detail="$3"
  printf '{"suite":"e2e_smoke","case":"%s","status":"%s","detail":"%s"}\n' "$name" "$status" "$detail" >>"$CASE_FILE"
}

fail_case() {
  record_case "$1" "fail" "$2"
  cat >"$REPORT_DIR/e2e_smoke.json" <<EOF
{"suite":"e2e_smoke","status":"fail","failed_case":"$1"}
EOF
  exit 1
}

pass_case() {
  record_case "$1" "pass" "$2"
}

mysql_query() {
  docker compose exec -T mysql mysql -N -uapp_user -papp_password_local hospital_platform -e "$1"
}

login_token() {
  local username="$1"
  local password="$2"
  local response
  response=$(curl -s -X POST "$API_BASE/auth/login" \
    -H "Content-Type: application/json" \
    -d "{\"username\":\"$username\",\"password\":\"$password\"}")
  local token
  token=$(python3 -c 'import json,sys
payload = {}
try:
    payload = json.load(sys.stdin)
except Exception:
    pass
print(payload.get("token", ""))' <<<"$response")
  printf '%s' "$token"
}

status_for() {
  local token="$1"
  local method="$2"
  local path="$3"
  curl -s -o /tmp/e2e_smoke_body.json -w "%{http_code}" -X "$method" "$API_BASE$path" -H "X-Session-Token: $token"
}

assert_status() {
  local case_name="$1"
  local expected="$2"
  local actual="$3"
  if [ "$actual" != "$expected" ]; then
    fail_case "$case_name" "expected $expected got $actual"
  fi
  pass_case "$case_name" "received $actual"
}

mysql_query "UPDATE users SET is_disabled = 0 WHERE username IN ('admin','employee1','member1','clinical1','cafeteria1');"

admin_token=$(login_token "admin" "Admin#OfflinePass123")
clinical_token=$(login_token "clinical1" "Admin#OfflinePass123")
cafeteria_token=$(login_token "cafeteria1" "Admin#OfflinePass123")
member_token=$(login_token "member1" "Admin#OfflinePass123")

[ -n "$admin_token" ] || fail_case "login_admin" "login failed; token missing"
[ -n "$clinical_token" ] || fail_case "login_clinical1" "login failed; token missing"
[ -n "$cafeteria_token" ] || fail_case "login_cafeteria1" "login failed; token missing"
[ -n "$member_token" ] || fail_case "login_member1" "login failed; token missing"

code=$(status_for "$admin_token" "GET" "/rbac/menu-entitlements")
assert_status "admin_journey_entitlements" "200" "$code"
code=$(status_for "$admin_token" "GET" "/orders")
assert_status "admin_journey_orders" "200" "$code"
code=$(status_for "$admin_token" "GET" "/ingestion/tasks")
assert_status "admin_journey_ingestion" "200" "$code"

code=$(status_for "$clinical_token" "GET" "/patients/search?q=john")
assert_status "clinical_journey_patients" "200" "$code"
code=$(curl -s -o /tmp/e2e_smoke_body.json -w "%{http_code}" -X POST "$API_BASE/cafeteria/dishes" -H "X-Session-Token: $clinical_token" -H "Content-Type: application/json" -d '{"category_id":1,"name":"forbidden","description":"forbidden","base_price_cents":100,"photo_path":"/tmp/a.jpg"}')
assert_status "clinical_journey_dining_management_denied" "403" "$code"
patient_id=$(mysql_query "SELECT id FROM patients ORDER BY id DESC LIMIT 1;")
if [ -n "$patient_id" ]; then
  clinical_user_id=$(mysql_query "SELECT id FROM users WHERE username='clinical1' LIMIT 1;")
  [ -n "$clinical_user_id" ] || fail_case "clinical_journey_export" "clinical1 user id not found"
  mysql_query "INSERT INTO patient_assignments (patient_id, user_id, assignment_type, assigned_by, assigned_at) VALUES ($patient_id, $clinical_user_id, 'care_team', $clinical_user_id, NOW()) ON DUPLICATE KEY UPDATE assignment_type = VALUES(assignment_type), assigned_by = VALUES(assigned_by), assigned_at = VALUES(assigned_at);"
  code=$(status_for "$clinical_token" "GET" "/patients/$patient_id/export?format=json")
  assert_status "clinical_journey_export" "200" "$code"
fi

code=$(status_for "$cafeteria_token" "GET" "/orders")
assert_status "cafeteria_journey_orders" "200" "$code"
code=$(status_for "$cafeteria_token" "GET" "/patients/search?q=john")
assert_status "cafeteria_journey_patient_denied" "403" "$code"
code=$(status_for "$cafeteria_token" "GET" "/cafeteria/categories")
assert_status "cafeteria_journey_inventory" "200" "$code"
if [ -n "$patient_id" ]; then
  code=$(status_for "$cafeteria_token" "GET" "/patients/$patient_id/export?format=json")
  assert_status "cafeteria_journey_export_denied" "403" "$code"
fi

code=$(status_for "$member_token" "GET" "/orders")
assert_status "member_journey_orders" "200" "$code"
code=$(status_for "$member_token" "GET" "/patients/search?q=john")
assert_status "member_journey_patient_denied" "403" "$code"
code=$(status_for "$member_token" "GET" "/ingestion/tasks")
assert_status "member_journey_ingestion_denied" "403" "$code"

code=$(status_for "$member_token" "GET" "/hospitals")
assert_status "member_journey_catalog_denied" "403" "$code"

cat >"$REPORT_DIR/e2e_smoke.json" <<EOF
{"suite":"e2e_smoke","status":"pass","cases":14}
EOF
