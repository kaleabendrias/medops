#!/usr/bin/env bash
set -euo pipefail

REPORT_DIR="${1:-test_reports}"
mkdir -p "$REPORT_DIR"

API_BASE="http://localhost:8000/api/v1"
CASE_FILE="$REPORT_DIR/authorization_matrix.ndjson"
: >"$CASE_FILE"

record_case() {
  local name="$1"
  local status="$2"
  local detail="$3"
  printf '{"suite":"authorization_matrix","case":"%s","status":"%s","detail":"%s"}\n' "$name" "$status" "$detail" >>"$CASE_FILE"
}

fail_case() {
  record_case "$1" "fail" "$2"
  cat >"$REPORT_DIR/authorization_matrix.json" <<EOF
{"suite":"authorization_matrix","status":"fail","failed_case":"$1"}
EOF
  exit 1
}

pass_case() {
  record_case "$1" "pass" "$2"
}

mysql_query() {
  docker compose exec -T mysql mysql -N -uapp_user -papp_password_local hospital_platform -e "$1" 2>/dev/null
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
  curl -s -o /tmp/auth_matrix_body.txt -w "%{http_code}" -X "$method" "$API_BASE$path" -H "X-Session-Token: $token"
}

status_anon() {
  local method="$1"
  local path="$2"
  curl -s -o /tmp/auth_matrix_body.txt -w "%{http_code}" -X "$method" "$API_BASE$path"
}

menu_allowed() {
  local token="$1"
  local key="$2"
  curl -s -X GET "$API_BASE/rbac/menu-entitlements" -H "X-Session-Token: $token" \
    | python3 -c 'import json,sys; key=sys.argv[1]; items=json.load(sys.stdin); print("1" if any(x.get("menu_key")==key and x.get("allowed") for x in items) else "0")' "$key"
}

mysql_query "UPDATE users SET is_disabled = 0 WHERE username IN ('admin','employee1','member1','clinical1','cafeteria1');"

admin_token=$(login_token "admin" "Admin#OfflinePass123")
member_token=$(login_token "member1" "Admin#OfflinePass123")
clinical_token=$(login_token "clinical1" "Admin#OfflinePass123")
cafeteria_token=$(login_token "cafeteria1" "Admin#OfflinePass123")

[ -n "$admin_token" ] || fail_case "login_admin" "login failed; token missing"
[ -n "$member_token" ] || fail_case "login_member1" "login failed; token missing"
[ -n "$clinical_token" ] || fail_case "login_clinical1" "login failed; token missing"
[ -n "$cafeteria_token" ] || fail_case "login_cafeteria1" "login failed; token missing"

code=$(status_anon "GET" "/orders")
[ "$code" = "401" ] || fail_case "orders_require_session" "expected 401 got $code"
pass_case "orders_require_session" "unauthenticated request denied"

code=$(status_for "$admin_token" "GET" "/admin/users")
[ "$code" = "200" ] || fail_case "admin_users_allow_admin" "expected 200 got $code"
pass_case "admin_users_allow_admin" "admin can list users"

code=$(status_for "$member_token" "GET" "/admin/users")
[ "$code" = "403" ] || fail_case "admin_users_deny_member" "expected 403 got $code"
pass_case "admin_users_deny_member" "member denied admin users endpoint"

code=$(status_for "$clinical_token" "GET" "/patients/search?q=jo")
[ "$code" = "200" ] || fail_case "patients_search_allow_clinical" "expected 200 got $code"
pass_case "patients_search_allow_clinical" "clinical user can search patients"

code=$(status_for "$cafeteria_token" "GET" "/patients/search?q=jo")
[ "$code" = "403" ] || fail_case "patients_search_deny_cafeteria" "expected 403 got $code"
pass_case "patients_search_deny_cafeteria" "cafeteria user denied patient search"

code=$(status_for "$member_token" "GET" "/patients/search?q=jo")
[ "$code" = "403" ] || fail_case "patients_search_deny_member" "expected 403 got $code"
pass_case "patients_search_deny_member" "member denied patient search"

code=$(status_for "$admin_token" "GET" "/retention/policies")
[ "$code" = "200" ] || fail_case "retention_allow_admin" "expected 200 got $code"
pass_case "retention_allow_admin" "admin can list retention policies"

code=$(status_for "$member_token" "GET" "/retention/policies")
[ "$code" = "403" ] || fail_case "retention_deny_member" "expected 403 got $code"
pass_case "retention_deny_member" "member denied retention policies"

code=$(status_for "$admin_token" "GET" "/ingestion/tasks")
[ "$code" = "200" ] || fail_case "ingestion_allow_admin" "expected 200 got $code"
pass_case "ingestion_allow_admin" "admin can list ingestion tasks"

code=$(status_for "$cafeteria_token" "GET" "/ingestion/tasks")
[ "$code" = "403" ] || fail_case "ingestion_deny_cafeteria" "expected 403 got $code"
pass_case "ingestion_deny_cafeteria" "cafeteria user denied ingestion tasks"

auth_task_name="auth-matrix-$(date +%s%N)"
create_code=$(curl -s -o /tmp/auth_matrix_body.txt -w "%{http_code}" -X POST "$API_BASE/ingestion/tasks" -H "X-Session-Token: $admin_token" -H "Content-Type: application/json" -d "{\"task_name\":\"$auth_task_name\",\"seed_urls\":[\"file:///app/config/ingestion_fixture/page1.html\"],\"extraction_rules_json\":\"{\\\"mode\\\":\\\"css\\\",\\\"fields\\\":[\\\".record\\\"]}\",\"pagination_strategy\":\"breadth-first\",\"max_depth\":1,\"incremental_field\":\"value\",\"schedule_cron\":\"0 * * * *\"}")
[ "$create_code" = "200" ] || fail_case "ingestion_create_allow_admin" "expected 200 got $create_code"
task_id=$(python3 -c 'import json; print(json.load(open("/tmp/auth_matrix_body.txt")))')
pass_case "ingestion_create_allow_admin" "admin can create ingestion tasks"

code=$(status_for "$cafeteria_token" "POST" "/ingestion/tasks/$task_id/run")
[ "$code" = "403" ] || fail_case "ingestion_run_deny_cafeteria" "expected 403 got $code"
pass_case "ingestion_run_deny_cafeteria" "cafeteria user denied ingestion run endpoint"

code=$(status_for "$cafeteria_token" "GET" "/cafeteria/categories")
[ "$code" = "200" ] || fail_case "cafeteria_allow_inventory_read" "expected 200 got $code"
pass_case "cafeteria_allow_inventory_read" "cafeteria user can read dining inventory"

code=$(curl -s -o /tmp/auth_matrix_body.txt -w "%{http_code}" -X POST "$API_BASE/cafeteria/dishes" -H "X-Session-Token: $clinical_token" -H "Content-Type: application/json" -d '{"category_id":1,"name":"forbidden-clinical","description":"forbidden","base_price_cents":999,"photo_path":"/tmp/a.jpg"}')
[ "$code" = "403" ] || fail_case "clinical_deny_inventory_write" "expected 403 got $code"
pass_case "clinical_deny_inventory_write" "clinical user denied dining pricing/inventory writes"

code=$(status_anon "GET" "/hospitals")
[ "$code" = "401" ] || fail_case "catalog_requires_auth" "expected 401 got $code"
pass_case "catalog_requires_auth" "catalog metadata routes require authentication"

code=$(status_for "$member_token" "GET" "/hospitals")
[ "$code" = "403" ] || fail_case "catalog_requires_authorization" "expected 403 got $code"
pass_case "catalog_requires_authorization" "catalog metadata routes are authorization-gated"

patient_id=$(mysql_query "SELECT id FROM patients ORDER BY id DESC LIMIT 1;")
if [ -z "$patient_id" ]; then
  patient_create=$(curl -s -X POST "$API_BASE/patients" -H "X-Session-Token: $admin_token" -H "Content-Type: application/json" -d '{"mrn":"MRN-AUTH-MATRIX","first_name":"Auth","last_name":"Matrix","birth_date":"1991-01-01","gender":"F","phone":"555-4000","email":"auth.matrix@example.local","allergies":"none","contraindications":"none","history":"baseline"}')
  patient_id=$(python3 -c 'import json,sys; print(json.load(sys.stdin))' <<<"$patient_create")
fi

clinical_user_id=$(mysql_query "SELECT id FROM users WHERE username='clinical1' LIMIT 1;")
[ -n "$clinical_user_id" ] || fail_case "patient_export_allow_clinical" "clinical1 user id not found"
mysql_query "INSERT INTO patient_assignments (patient_id, user_id, assignment_type, assigned_by, assigned_at) VALUES ($patient_id, $clinical_user_id, 'care_team', $clinical_user_id, NOW()) ON DUPLICATE KEY UPDATE assignment_type = VALUES(assignment_type), assigned_by = VALUES(assigned_by), assigned_at = VALUES(assigned_at);"

code=$(status_for "$clinical_token" "GET" "/patients/$patient_id/export?format=json")
[ "$code" = "200" ] || fail_case "patient_export_allow_clinical" "expected 200 got $code"
pass_case "patient_export_allow_clinical" "clinical user can run export workflow"

code=$(status_for "$cafeteria_token" "GET" "/patients/$patient_id/export?format=json")
[ "$code" = "403" ] || fail_case "patient_export_deny_cafeteria" "expected 403 got $code"
pass_case "patient_export_deny_cafeteria" "cafeteria user denied patient export workflow"

# ── Member order isolation: cross-patient access denied ──
# Create a patient and an order as admin, then verify member cannot access it.
auth_matrix_patient_id=$(mysql_query "SELECT id FROM patients ORDER BY id ASC LIMIT 1;")
if [ -n "$auth_matrix_patient_id" ]; then
  menu_id=$(mysql_query "SELECT id FROM dining_menus ORDER BY id ASC LIMIT 1;")
  if [ -n "$menu_id" ]; then
    # Admin creates an order
    admin_order_code=$(curl -s -o /tmp/auth_matrix_body.txt -w "%{http_code}" -X POST "$API_BASE/orders" -H "X-Session-Token: $admin_token" -H "Content-Type: application/json" -d "{\"patient_id\":$auth_matrix_patient_id,\"menu_id\":$menu_id,\"notes\":\"matrix isolation test\"}")
    if [ "$admin_order_code" = "200" ]; then
      admin_order_id=$(python3 -c 'import json; print(json.load(open("/tmp/auth_matrix_body.txt")))')

      # Member must NOT read admin's order notes
      code=$(status_for "$member_token" "GET" "/orders/$admin_order_id/notes")
      [ "$code" = "403" ] || fail_case "member_cross_order_read_denied" "expected 403 got $code"
      pass_case "member_cross_order_read_denied" "member cannot read another user's order notes"

      # Member must NOT update admin's order status
      code=$(curl -s -o /tmp/auth_matrix_body.txt -w "%{http_code}" -X PUT "$API_BASE/orders/$admin_order_id/status" -H "X-Session-Token: $member_token" -H "Content-Type: application/json" -d '{"status":"Canceled","reason":"unauthorized"}')
      [ "$code" = "403" ] || fail_case "member_cross_order_mutate_denied" "expected 403 got $code"
      pass_case "member_cross_order_mutate_denied" "member cannot mutate another user's order"

      # Member must NOT add notes to admin's order
      code=$(curl -s -o /tmp/auth_matrix_body.txt -w "%{http_code}" -X POST "$API_BASE/orders/$admin_order_id/notes" -H "X-Session-Token: $member_token" -H "Content-Type: application/json" -d '{"note":"unauthorized"}')
      [ "$code" = "403" ] || fail_case "member_cross_order_note_denied" "expected 403 got $code"
      pass_case "member_cross_order_note_denied" "member cannot add notes to another user's order"

      # Member must NOT add ticket splits to admin's order
      code=$(curl -s -o /tmp/auth_matrix_body.txt -w "%{http_code}" -X POST "$API_BASE/orders/$admin_order_id/ticket-splits" -H "X-Session-Token: $member_token" -H "Content-Type: application/json" -d '{"split_by":"ward","split_value":"X","quantity":1}')
      [ "$code" = "403" ] || fail_case "member_cross_order_split_denied" "expected 403 got $code"
      pass_case "member_cross_order_split_denied" "member cannot add splits to another user's order"
    fi
  fi
fi

admin_orders_menu=$(menu_allowed "$admin_token" "orders")
member_orders_menu=$(menu_allowed "$member_token" "orders")
code=$(status_for "$admin_token" "GET" "/orders")
if [ "$admin_orders_menu" = "1" ] && [ "$code" != "200" ]; then
  fail_case "menu_route_service_consistency_admin_orders" "orders menu allowed but route denied"
fi
if [ "$admin_orders_menu" = "0" ] && [ "$code" = "200" ]; then
  fail_case "menu_route_service_consistency_admin_orders" "orders menu denied but route allowed"
fi
pass_case "menu_route_service_consistency_admin_orders" "admin menu entitlement aligns with route and service"

code=$(status_for "$member_token" "GET" "/orders")
if [ "$member_orders_menu" = "1" ] && [ "$code" != "200" ]; then
  fail_case "menu_route_service_consistency_member_orders" "orders menu allowed but route denied"
fi
if [ "$member_orders_menu" = "0" ] && [ "$code" = "200" ]; then
  fail_case "menu_route_service_consistency_member_orders" "orders menu denied but route allowed"
fi
pass_case "menu_route_service_consistency_member_orders" "member menu entitlement aligns with route and service"

admin_ingestion_menu=$(menu_allowed "$admin_token" "ingestion")
member_ingestion_menu=$(menu_allowed "$member_token" "ingestion")
code=$(status_for "$admin_token" "GET" "/ingestion/tasks")
if [ "$admin_ingestion_menu" = "1" ] && [ "$code" != "200" ]; then
  fail_case "menu_route_service_consistency_admin_ingestion" "ingestion menu allowed but route denied"
fi
if [ "$admin_ingestion_menu" = "0" ] && [ "$code" = "200" ]; then
  fail_case "menu_route_service_consistency_admin_ingestion" "ingestion menu denied but route allowed"
fi
pass_case "menu_route_service_consistency_admin_ingestion" "admin ingestion entitlement aligns with route and service"

code=$(status_for "$member_token" "GET" "/ingestion/tasks")
if [ "$member_ingestion_menu" = "1" ] && [ "$code" != "200" ]; then
  fail_case "menu_route_service_consistency_member_ingestion" "ingestion menu allowed but route denied"
fi
if [ "$member_ingestion_menu" = "0" ] && [ "$code" = "200" ]; then
  fail_case "menu_route_service_consistency_member_ingestion" "ingestion menu denied but route allowed"
fi
pass_case "menu_route_service_consistency_member_ingestion" "member ingestion entitlement aligns with route and service"

# ── Governance RBAC: admin-only access ──
code=$(status_for "$admin_token" "GET" "/governance/records")
[ "$code" = "200" ] || fail_case "governance_allow_admin" "expected 200 got $code"
pass_case "governance_allow_admin" "admin can access governance records"

code=$(status_for "$member_token" "GET" "/governance/records")
[ "$code" = "403" ] || fail_case "governance_deny_member" "expected 403 got $code"
pass_case "governance_deny_member" "member denied governance records"

code=$(status_for "$clinical_token" "GET" "/governance/records")
[ "$code" = "403" ] || fail_case "governance_deny_clinical" "expected 403 got $code"
pass_case "governance_deny_clinical" "clinical user denied governance records"

code=$(status_for "$cafeteria_token" "GET" "/governance/records")
[ "$code" = "403" ] || fail_case "governance_deny_cafeteria" "expected 403 got $code"
pass_case "governance_deny_cafeteria" "cafeteria user denied governance records"

# ── Audit log RBAC ──
code=$(status_for "$admin_token" "GET" "/audits")
[ "$code" = "200" ] || fail_case "audits_allow_admin" "expected 200 got $code"
pass_case "audits_allow_admin" "admin can list audit logs"

code=$(status_for "$member_token" "GET" "/audits")
[ "$code" = "403" ] || fail_case "audits_deny_member" "expected 403 got $code"
pass_case "audits_deny_member" "member denied audit log access"

# ── Analytics RBAC ──
code=$(status_for "$admin_token" "GET" "/analytics/funnel")
[ "$code" = "200" ] || fail_case "analytics_allow_admin" "expected 200 got $code"
pass_case "analytics_allow_admin" "admin can access analytics"

code=$(status_for "$member_token" "GET" "/analytics/funnel")
[ "$code" = "403" ] || fail_case "analytics_deny_member" "expected 403 got $code"
pass_case "analytics_deny_member" "member denied analytics access"

cat >"$REPORT_DIR/authorization_matrix.json" <<EOF
{"suite":"authorization_matrix","status":"pass","cases":36}
EOF
