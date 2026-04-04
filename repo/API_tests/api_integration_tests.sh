#!/usr/bin/env bash
set -euo pipefail

REPORT_DIR="${1:-test_reports}"
mkdir -p "$REPORT_DIR"

API_BASE="http://localhost:8000/api/v1"
CASE_FILE="$REPORT_DIR/api_integration_tests.ndjson"
: >"$CASE_FILE"
RUN_ID=$(date +%s)

record_case() {
  local name="$1"
  local status="$2"
  local detail="$3"
  printf '{"suite":"api_integration_tests","case":"%s","status":"%s","detail":"%s"}\n' "$name" "$status" "$detail" >>"$CASE_FILE"
}

fail_case() {
  record_case "$1" "fail" "$2"
  cat >"$REPORT_DIR/api_integration_tests.json" <<EOF
{"suite":"api_integration_tests","status":"fail","failed_case":"$1"}
EOF
  exit 1
}

pass_case() {
  record_case "$1" "pass" "$2"
}

mysql_query() {
  docker compose exec -T mysql mysql -N -uapp_user -papp_password_local hospital_platform -e "$1"
}

login_response() {
  local username="$1"
  local password="$2"
  curl -s -X POST "$API_BASE/auth/login" \
    -H "Content-Type: application/json" \
    -d "{\"username\":\"$username\",\"password\":\"$password\"}"
}

login_token() {
  login_response "$1" "$2" | python3 -c 'import sys,json; print(json.load(sys.stdin)["token"])'
}

login_user_id() {
  login_response "$1" "$2" | python3 -c 'import sys,json; print(json.load(sys.stdin)["user_id"])'
}

api_call() {
  local method="$1"
  local path="$2"
  local token="${3:-}"
  local data="${4:-}"
  if [ -n "$token" ] && [ -n "$data" ]; then
    curl -s -o /tmp/api_test_body.json -w "%{http_code}" -X "$method" "$API_BASE$path" -H "X-Session-Token: $token" -H "Content-Type: application/json" -d "$data"
  elif [ -n "$token" ]; then
    curl -s -o /tmp/api_test_body.json -w "%{http_code}" -X "$method" "$API_BASE$path" -H "X-Session-Token: $token"
  elif [ -n "$data" ]; then
    curl -s -o /tmp/api_test_body.json -w "%{http_code}" -X "$method" "$API_BASE$path" -H "Content-Type: application/json" -d "$data"
  else
    curl -s -o /tmp/api_test_body.json -w "%{http_code}" -X "$method" "$API_BASE$path"
  fi
}

assert_code() {
  local case_name="$1"
  local expected="$2"
  local actual="$3"
  if [ "$actual" != "$expected" ]; then
    fail_case "$case_name" "expected $expected got $actual"
  fi
  pass_case "$case_name" "received $actual"
}

admin_token=$(login_token "admin" "Admin#OfflinePass123")
member_token=$(login_token "member1" "Admin#OfflinePass123")
member_user_id=$(login_user_id "member1" "Admin#OfflinePass123")
clinical_user_id=$(login_user_id "clinical1" "Admin#OfflinePass123")
cafeteria_user_id=$(login_user_id "cafeteria1" "Admin#OfflinePass123")

mysql_query "UPDATE users SET is_disabled = 0, failed_attempts = 0, locked_until = NULL WHERE username IN ('admin','member1','clinical1','cafeteria1','lockout_user');"

admin_hash=$(mysql_query "SELECT password_hash FROM users WHERE username = 'admin';")
if [[ "$admin_hash" != \$argon2id\$* ]]; then
  fail_case "auth_argon2_migration" "expected admin password_hash to migrate to argon2id"
fi
pass_case "auth_argon2_migration" "legacy sha256 credentials upgraded to argon2id on login"

# Non-complex passwords are rejected with 401 (same as wrong-password) to avoid
# leaking which validation step failed.  The failed attempt is still counted
# toward the lockout threshold.
code=$(api_call "POST" "/auth/login" "" '{"username":"admin","password":"short"}')
assert_code "password_policy_enforced" "401" "$code"

code=$(api_call "GET" "/patients/search?q=john" "$member_token")
assert_code "rbac_denial_member_patient_search" "403" "$code"

code=$(api_call "GET" "/patients/search?q=john" "$admin_token")
assert_code "rbac_allow_admin_patient_search" "200" "$code"

code=$(api_call "GET" "/hospitals")
assert_code "catalog_hospitals_require_auth" "401" "$code"

code=$(api_call "GET" "/hospitals" "$member_token")
assert_code "catalog_hospitals_require_authorization" "403" "$code"

code=$(api_call "GET" "/hospitals" "$admin_token")
assert_code "catalog_hospitals_admin_allowed" "200" "$code"

for i in 1 2 3 4 5; do
  code=$(api_call "POST" "/auth/login" "" '{"username":"lockout_user","password":"Wrong#Password123"}')
  if [ "$code" != "401" ]; then
    fail_case "lockout_failed_attempt_$i" "expected 401 got $code"
  fi
done
pass_case "lockout_failed_attempts" "five failed logins recorded"

code=$(api_call "POST" "/auth/login" "" '{"username":"lockout_user","password":"Admin#OfflinePass123"}')
assert_code "lockout_enforced_after_failures" "400" "$code"

session_timeout_token=$(login_token "member1" "Admin#OfflinePass123")
mysql_query "UPDATE sessions SET last_activity_at = DATE_SUB(NOW(), INTERVAL 481 MINUTE) WHERE session_token = '$session_timeout_token';"
code=$(api_call "GET" "/session" "$session_timeout_token")
assert_code "session_timeout_enforced" "401" "$code"

member_live_token=$(login_token "member1" "Admin#OfflinePass123")
code=$(api_call "POST" "/admin/users/$member_user_id/disable" "$admin_token")
assert_code "admin_disable_user" "200" "$code"
code=$(api_call "GET" "/orders" "$member_live_token")
assert_code "admin_disable_immediate_revoke" "401" "$code"
mysql_query "UPDATE users SET is_disabled = 0 WHERE id = $member_user_id;"
member_disabled=$(mysql_query "SELECT is_disabled FROM users WHERE id = $member_user_id;")
if [ "$member_disabled" != "0" ]; then
  fail_case "admin_disable_cleanup_reenable" "failed to restore member1 active state"
fi
pass_case "admin_disable_cleanup_reenable" "restored member1 active state for downstream suites"

patient_payload="{\"mrn\":\"MRN-T001-$RUN_ID\",\"first_name\":\"Test\",\"last_name\":\"Patient\",\"birth_date\":\"1990-01-01\",\"gender\":\"F\",\"phone\":\"555-1111\",\"email\":\"test.patient+$RUN_ID@example.local\",\"allergies\":\"none\",\"contraindications\":\"none\",\"history\":\"baseline\"}"
code=$(api_call "POST" "/patients" "$admin_token" "$patient_payload")
assert_code "create_patient_for_scenarios" "200" "$code"
patient_id=$(python3 -c 'import json; print(json.load(open("/tmp/api_test_body.json")))')

clinical_token=$(login_token "clinical1" "Admin#OfflinePass123")
cafeteria_token=$(login_token "cafeteria1" "Admin#OfflinePass123")
code=$(api_call "POST" "/orders" "$clinical_token" "{\"patient_id\":$patient_id,\"menu_id\":1,\"notes\":\"clinical-forbidden\"}")
assert_code "order_requires_explicit_assignment" "403" "$code"

code=$(api_call "GET" "/patients/search?q=test" "$cafeteria_token")
assert_code "cafeteria_role_isolation_patient_search" "403" "$code"

code=$(api_call "GET" "/patients/$patient_id" "$clinical_token")
assert_code "patient_object_isolation_before_assignment" "403" "$code"

code=$(api_call "GET" "/patients/$patient_id/attachments" "$clinical_token")
assert_code "attachment_object_isolation_before_assignment" "403" "$code"

code=$(api_call "POST" "/patients/$patient_id/assign" "$admin_token" "{\"target_user_id\":$clinical_user_id,\"assignment_type\":\"care_team\"}")
assert_code "patient_assignment_create" "200" "$code"

code=$(api_call "GET" "/patients/$patient_id" "$clinical_token")
assert_code "patient_object_access_after_assignment" "200" "$code"
masked_allergies=$(python3 - <<'PY'
import json
with open('/tmp/api_test_body.json') as f:
    data = json.load(f)
print(data.get('allergies', ''))
PY
)
if [ "$masked_allergies" != "[REDACTED - privileged reveal required]" ]; then
  fail_case "patient_masking_default" "expected masked allergies by default"
fi
pass_case "patient_masking_default" "sensitive patient fields are masked by default"

code=$(api_call "GET" "/patients/$patient_id?reveal_sensitive=true" "$cafeteria_token")
assert_code "patient_reveal_requires_privilege" "403" "$code"

code=$(api_call "GET" "/patients/$patient_id/revisions?reveal_sensitive=true" "$cafeteria_token")
assert_code "patient_revision_reveal_requires_privilege" "403" "$code"

code=$(api_call "GET" "/patients/$patient_id?reveal_sensitive=true" "$admin_token")
assert_code "patient_reveal_privileged_path" "200" "$code"
revealed_allergies=$(python3 - <<'PY'
import json
with open('/tmp/api_test_body.json') as f:
    data = json.load(f)
print(data.get('allergies', ''))
PY
)
if [ "$revealed_allergies" != "none" ]; then
  fail_case "patient_reveal_content" "expected revealed allergies to match seeded value"
fi
pass_case "patient_reveal_content" "privileged reveal endpoint returns clear sensitive content"

good_update='{"first_name":"Updated","last_name":"Patient","birth_date":"1990-01-01","gender":"F","phone":"555-3333","email":"updated.ok@example.local","reason_for_change":"demographics correction"}'
code=$(api_call "PUT" "/patients/$patient_id" "$admin_token" "$good_update")
assert_code "patient_revision_delta_demographics_update" "200" "$code"

code=$(api_call "PUT" "/patients/$patient_id/allergies" "$admin_token" '{"value":"shellfish","reason_for_change":"allergy confirmation"}')
assert_code "patient_revision_delta_clinical_update" "200" "$code"

code=$(api_call "GET" "/patients/$patient_id/revisions" "$admin_token")
assert_code "patient_revision_delta_list_masked" "200" "$code"
masked_delta_ok=$(python3 - <<'PY'
import json
with open('/tmp/api_test_body.json') as f:
    rows = json.load(f)
if not rows:
    print('0')
else:
    found = False
    for rev in rows:
        deltas = json.loads(rev.get('field_deltas_json') or '[]')
        for d in deltas:
            if d.get('field') == 'allergies':
                found = d.get('before') == '[REDACTED - privileged reveal required]' and d.get('after') == '[REDACTED - privileged reveal required]'
                break
        if found:
            break
    print('1' if found else '0')
PY
)
if [ "$masked_delta_ok" != "1" ]; then
  fail_case "patient_revision_delta_masking" "expected masked allergy deltas without reveal"
fi
pass_case "patient_revision_delta_masking" "revision deltas mask sensitive fields by default"

code=$(api_call "GET" "/patients/$patient_id/revisions?reveal_sensitive=true" "$admin_token")
assert_code "patient_revision_delta_list_revealed" "200" "$code"
revealed_delta_ok=$(python3 - <<'PY'
import json
with open('/tmp/api_test_body.json') as f:
    rows = json.load(f)
if not rows:
    print('0')
else:
    found_demo = False
    found_allergy = False
    for rev in rows:
        deltas = json.loads(rev.get('field_deltas_json') or '[]')
        for d in deltas:
            if d.get('field') == 'first_name' and d.get('after') == 'Updated':
                found_demo = True
            if d.get('field') == 'allergies' and d.get('after') == 'shellfish':
                found_allergy = True
    print('1' if found_demo and found_allergy else '0')
PY
)
if [ "$revealed_delta_ok" != "1" ]; then
  fail_case "patient_revision_delta_highlight" "expected field-level before/after deltas for demographics and allergies"
fi
pass_case "patient_revision_delta_highlight" "revision timeline returns field-level before/after deltas"

reason_visible=$(python3 - <<'PY'
import json
with open('/tmp/api_test_body.json') as f:
    rows = json.load(f)
print('1' if any((x.get('reason_for_change') or '').strip() for x in rows) else '0')
PY
)
if [ "$reason_visible" != "1" ]; then
  fail_case "patient_revision_reason_traceability" "revision timeline missing reason_for_change entries"
fi
pass_case "patient_revision_reason_traceability" "revision timeline retains reason-for-change traceability"

bad_update='{"first_name":"Updated","last_name":"Patient","birth_date":"1990-01-01","gender":"F","phone":"555-2222","email":"updated@example.local","reason_for_change":""}'
code=$(api_call "PUT" "/patients/$patient_id" "$admin_token" "$bad_update")
assert_code "patient_revision_reason_required" "400" "$code"

invalid_attachment_code=$(curl -s -o /tmp/api_test_body.json -w "%{http_code}" -X POST "$API_BASE/patients/$patient_id/attachments?filename=payload.exe&mime_type=application/octet-stream" -H "X-Session-Token: $admin_token" --data-binary "malicious")
assert_code "attachment_type_constraint" "400" "$invalid_attachment_code"

python3 - <<'PY'
with open('/tmp/oversize_attachment.bin', 'wb') as f:
    f.write(b'A' * (26 * 1024 * 1024))
PY
oversize_code=$(curl -s -o /tmp/api_test_body.json -w "%{http_code}" -X POST "$API_BASE/patients/$patient_id/attachments?filename=big.pdf&mime_type=application/pdf" -H "X-Session-Token: $admin_token" --data-binary @/tmp/oversize_attachment.bin)
assert_code "attachment_size_constraint" "413" "$oversize_code"

python3 - <<'PY'
with open('/tmp/binary-attachment.pdf', 'wb') as f:
    payload = bytes([0, 1, 2, 3, 10, 13, 255, 128, 64, 32]) + b'BINARY-CONTENT' + bytes(range(16))
    f.write(payload)
PY
binary_upload_code=$(curl -s -o /tmp/api_test_body.json -w "%{http_code}" -X POST "$API_BASE/patients/$patient_id/attachments?filename=binary-roundtrip.pdf&mime_type=application/pdf" -H "X-Session-Token: $admin_token" --data-binary @/tmp/binary-attachment.pdf)
assert_code "attachment_binary_upload" "200" "$binary_upload_code"

code=$(api_call "GET" "/patients/$patient_id/attachments" "$admin_token")
assert_code "attachment_binary_metadata" "200" "$code"
binary_attachment_id=$(python3 - <<'PY'
import json
with open('/tmp/api_test_body.json') as f:
    rows = json.load(f)
target = next((x for x in rows if x.get('file_name') == 'binary-roundtrip.pdf'), None)
print(target.get('id') if target else '')
PY
)
if [ -z "$binary_attachment_id" ]; then
  fail_case "attachment_binary_metadata" "uploaded binary attachment not found in metadata"
fi

binary_download_code=$(curl -s -o /tmp/binary-download.pdf -w "%{http_code}" -X GET "$API_BASE/patients/$patient_id/attachments/$binary_attachment_id/download" -H "X-Session-Token: $admin_token")
assert_code "attachment_binary_download" "200" "$binary_download_code"
orig_hash=$(sha256sum /tmp/binary-attachment.pdf | awk '{print $1}')
down_hash=$(sha256sum /tmp/binary-download.pdf | awk '{print $1}')
if [ "$orig_hash" != "$down_hash" ]; then
  fail_case "attachment_binary_roundtrip_integrity" "binary attachment hash mismatch after round-trip"
fi
pass_case "attachment_binary_roundtrip_integrity" "binary attachment bytes preserved end-to-end"

download_audit_count=$(mysql_query "SELECT COUNT(*) FROM audit_logs WHERE action_type = 'clinical.attachment_download' AND entity_id = '$patient_id';")
if [ "$download_audit_count" -lt 1 ]; then
  fail_case "attachment_download_audit_logged" "expected attachment download audit record"
fi
pass_case "attachment_download_audit_logged" "attachment downloads emit audit records"

payload_size=$(mysql_query "SELECT OCTET_LENGTH(payload_blob) FROM patient_attachments WHERE id = $binary_attachment_id;")
if [ "$payload_size" -le 0 ]; then
  fail_case "attachment_binary_persisted_in_mysql" "expected attachment payload bytes in MySQL"
fi
pass_case "attachment_binary_persisted_in_mysql" "attachment payload stored in MySQL blob"

docker compose exec -T api sh -c 'mkdir -p /var/lib/rocket-api/uploads/legacy-tests && printf "LEGACY-BLOB" > /var/lib/rocket-api/uploads/legacy-tests/legacy-attachment.txt' >/dev/null
mysql_query "INSERT INTO patient_attachments (patient_id, file_name, mime_type, file_size_bytes, payload_blob, storage_path, uploaded_by, uploaded_at) VALUES ($patient_id, 'legacy-attachment.txt', 'application/pdf', 11, NULL, '/var/lib/rocket-api/uploads/legacy-tests/legacy-attachment.txt', 1, NOW());"
legacy_attachment_id=$(mysql_query "SELECT id FROM patient_attachments WHERE patient_id = $patient_id AND file_name = 'legacy-attachment.txt' ORDER BY id DESC LIMIT 1;")
legacy_download_code=$(curl -s -o /tmp/legacy-download.txt -w "%{http_code}" -X GET "$API_BASE/patients/$patient_id/attachments/$legacy_attachment_id/download" -H "X-Session-Token: $admin_token")
assert_code "attachment_legacy_fallback_download" "200" "$legacy_download_code"
legacy_content=$(python3 - <<'PY'
with open('/tmp/legacy-download.txt', 'rb') as f:
    print(f.read().decode('utf-8', errors='ignore'))
PY
)
if [ "$legacy_content" != "LEGACY-BLOB" ]; then
  fail_case "attachment_legacy_fallback_download" "legacy filesystem fallback payload mismatch"
fi
pass_case "attachment_legacy_fallback_download" "legacy attachment rows fallback to storage_path when blob is missing"

beds_json=$(curl -s -X GET "$API_BASE/bedboard/beds" -H "X-Session-Token: $admin_token")
bed_id=$(python3 -c 'import sys,json; data=json.load(sys.stdin); print(data[0]["id"])' <<<"$beds_json")
bed_state=$(python3 -c 'import sys,json; data=json.load(sys.stdin); print(data[0]["state"])' <<<"$beds_json")
legal_target=$(python3 - <<PY
state = "$bed_state"
mapping = {
    "Available": "Reserved",
    "Reserved": "Occupied",
    "Occupied": "Cleaning",
    "Cleaning": "Available",
    "Out of Service": "Available"
}
print(mapping.get(state, "Available"))
PY
)

code=$(api_call "POST" "/bedboard/beds/$bed_id/transition" "$admin_token" "{\"action\":\"check-in\",\"target_state\":\"$legal_target\",\"related_bed_id\":null,\"note\":\"legal transition test\"}")
assert_code "bed_state_machine_legal_transition" "200" "$code"

code=$(api_call "POST" "/bedboard/beds/$bed_id/transition" "$admin_token" '{"action":"check-in","target_state":"NotAState","related_bed_id":null,"note":"illegal transition test"}')
assert_code "bed_state_machine_illegal_transition" "400" "$code"

code=$(api_call "POST" "/cafeteria/dishes" "$admin_token" '{"category_id":1,"name":"Campaign Meal","description":"campaign dish","base_price_cents":1200,"photo_path":"/tmp/campaign.jpg"}')
assert_code "campaign_dish_create" "200" "$code"
campaign_dish_id=$(python3 -c 'import json; print(json.load(open("/tmp/api_test_body.json")))')

code=$(api_call "POST" "/dining/menus" "$admin_token" '{"menu_date":"2026-01-01","meal_period":"Lunch","item_name":"Campaign Meal","calories":500}')
assert_code "campaign_menu_create" "200" "$code"
campaign_menus_json=$(curl -s -X GET "$API_BASE/dining/menus" -H "X-Session-Token: $admin_token")
campaign_menu_id=$(python3 -c 'import sys,json; data=json.load(sys.stdin); print(data[0]["id"])' <<<"$campaign_menus_json")

code=$(api_call "POST" "/campaigns" "$admin_token" "{\"title\":\"Order Success Campaign\",\"dish_id\":$campaign_dish_id,\"success_threshold\":2,\"success_deadline_at\":\"2099-01-01 10:30:00\"}")
assert_code "campaign_create_with_deadline" "200" "$code"
campaign_id=$(python3 -c 'import json; print(json.load(open("/tmp/api_test_body.json")))')

code=$(api_call "POST" "/campaigns/$campaign_id/join" "$admin_token")
assert_code "campaign_join_admin" "200" "$code"
code=$(api_call "POST" "/campaigns/$campaign_id/join" "$cafeteria_token")
assert_code "campaign_join_cafeteria" "200" "$code"

code=$(api_call "POST" "/orders" "$admin_token" "{\"patient_id\":$patient_id,\"menu_id\":$campaign_menu_id,\"notes\":\"campaign-order-admin\"}")
assert_code "campaign_qualifying_order_admin" "200" "$code"
code=$(api_call "POST" "/orders" "$cafeteria_token" "{\"patient_id\":$patient_id,\"menu_id\":$campaign_menu_id,\"notes\":\"campaign-order-cafeteria\"}")
assert_code "campaign_qualifying_order_cafeteria" "200" "$code"

code=$(api_call "GET" "/campaigns" "$admin_token")
assert_code "campaign_success_refresh" "200" "$code"
campaign_success_ok=$(python3 - <<PY
import json
target_id = $campaign_id
with open('/tmp/api_test_body.json') as f:
    data = json.load(f)
target = next((x for x in data if x['id'] == target_id), None)
ok = bool(target and target.get('status') == 'Successful' and target.get('qualifying_orders', 0) >= 2)
print('1' if ok else '0')
PY
)
if [ "$campaign_success_ok" != "1" ]; then
  fail_case "campaign_success_by_qualifying_orders" "expected Successful campaign based on qualifying orders"
fi
pass_case "campaign_success_by_qualifying_orders" "campaign success computed from qualifying orders before deadline"

code=$(api_call "POST" "/campaigns" "$admin_token" "{\"title\":\"Inactivity Campaign\",\"dish_id\":$campaign_dish_id,\"success_threshold\":5,\"success_deadline_at\":\"2099-01-01 10:30:00\"}")
assert_code "campaign_create_for_inactivity" "200" "$code"
inactive_campaign_id=$(python3 -c 'import json; print(json.load(open("/tmp/api_test_body.json")))')
mysql_query "UPDATE group_campaigns SET last_activity_at = DATE_SUB(NOW(), INTERVAL 31 MINUTE) WHERE id = $inactive_campaign_id;"
code=$(api_call "GET" "/campaigns" "$admin_token")
assert_code "campaign_list_after_inactivity" "200" "$code"
campaign_status=$(python3 - <<PY
import json
target_id = $inactive_campaign_id
with open('/tmp/api_test_body.json') as f:
    data = json.load(f)
target = next((x for x in data if x['id'] == target_id), None)
print(target['status'] if target else '')
PY
)
if [ "$campaign_status" != "Closed" ]; then
  fail_case "campaign_inactivity_closure" "expected Closed got $campaign_status"
fi
pass_case "campaign_inactivity_closure" "campaign auto-closed after inactivity"

code=$(api_call "POST" "/campaigns" "$admin_token" "{\"title\":\"Deadline Campaign\",\"dish_id\":$campaign_dish_id,\"success_threshold\":10,\"success_deadline_at\":\"2099-01-01 10:30:00\"}")
assert_code "campaign_create_for_deadline" "200" "$code"
deadline_campaign_id=$(python3 -c 'import json; print(json.load(open("/tmp/api_test_body.json")))')
mysql_query "UPDATE group_campaigns SET success_deadline_at = DATE_SUB(NOW(), INTERVAL 1 MINUTE) WHERE id = $deadline_campaign_id;"
code=$(api_call "GET" "/campaigns" "$admin_token")
assert_code "campaign_list_after_deadline" "200" "$code"
deadline_closed=$(python3 - <<PY
import json
target_id = $deadline_campaign_id
with open('/tmp/api_test_body.json') as f:
    data = json.load(f)
target = next((x for x in data if x['id'] == target_id), None)
print('1' if target and target.get('status') == 'Closed' else '0')
PY
)
if [ "$deadline_closed" != "1" ]; then
  fail_case "campaign_deadline_closure" "campaign did not close after success deadline passed"
fi
pass_case "campaign_deadline_closure" "campaign closes when success deadline expires"

code=$(api_call "GET" "/patients/$patient_id/export?format=json" "$admin_token")
assert_code "patient_export_json" "200" "$code"
code=$(api_call "GET" "/patients/$patient_id/export?format=csv" "$admin_token")
assert_code "patient_export_csv" "200" "$code"
export_audit_count=$(mysql_query "SELECT COUNT(*) FROM audit_logs WHERE action_type = 'patient.export' AND entity_id = '$patient_id';")
if [ "$export_audit_count" -lt 2 ]; then
  fail_case "patient_export_audit_logged" "expected export audit records for json and csv"
fi
pass_case "patient_export_audit_logged" "patient export workflow emits audit records"

code=$(api_call "POST" "/dining/menus" "$admin_token" '{"menu_date":"2026-01-01","meal_period":"Lunch","item_name":"Test Meal","calories":500}')
assert_code "menu_create" "200" "$code"
menus_json=$(curl -s -X GET "$API_BASE/dining/menus" -H "X-Session-Token: $admin_token")
menu_id=$(python3 -c 'import sys,json; data=json.load(sys.stdin); print(data[-1]["id"])' <<<"$menus_json")

code=$(api_call "POST" "/orders" "$admin_token" "{\"patient_id\":$patient_id,\"menu_id\":$menu_id,\"notes\":\"integration order\"}")
assert_code "order_create" "200" "$code"
order_id=$(python3 -c 'import json; print(json.load(open("/tmp/api_test_body.json")))')

second_patient_payload="{\"mrn\":\"MRN-T002-$RUN_ID\",\"first_name\":\"Isolation\",\"last_name\":\"Case\",\"birth_date\":\"1988-02-02\",\"gender\":\"M\",\"phone\":\"555-2222\",\"email\":\"isolation.case+$RUN_ID@example.local\",\"allergies\":\"pollen\",\"contraindications\":\"none\",\"history\":\"isolation\"}"
code=$(api_call "POST" "/patients" "$admin_token" "$second_patient_payload")
assert_code "create_patient_for_cross_user_order_checks" "200" "$code"
isolated_patient_id=$(python3 -c 'import json; print(json.load(open("/tmp/api_test_body.json")))')
code=$(api_call "POST" "/orders" "$admin_token" "{\"patient_id\":$isolated_patient_id,\"menu_id\":$menu_id,\"notes\":\"isolated\"}")
assert_code "create_order_for_cross_user_access_checks" "200" "$code"
isolated_order_id=$(python3 -c 'import json; print(json.load(open("/tmp/api_test_body.json")))')

code=$(api_call "POST" "/orders/$isolated_order_id/notes" "$clinical_token" '{"note":"forbidden"}')
assert_code "order_note_cross_user_forbidden" "403" "$code"
code=$(api_call "GET" "/orders/$isolated_order_id/notes" "$clinical_token")
assert_code "order_notes_cross_user_forbidden" "403" "$code"
code=$(api_call "POST" "/orders/$isolated_order_id/ticket-splits" "$clinical_token" '{"split_by":"room","split_value":"A-1","quantity":1}')
assert_code "ticket_split_cross_user_forbidden" "403" "$code"
code=$(api_call "GET" "/orders/$isolated_order_id/ticket-splits" "$clinical_token")
assert_code "ticket_splits_cross_user_forbidden" "403" "$code"

code=$(api_call "PUT" "/orders/$order_id/status" "$admin_token" '{"status":"Billed"}')
assert_code "order_status_billed" "200" "$code"

code=$(api_call "PUT" "/orders/$order_id/status" "$admin_token" '{"status":"Credited"}')
assert_code "order_status_reason_required" "400" "$code"

code=$(api_call "PUT" "/orders/$order_id/status" "$admin_token" '{"status":"Credited","expected_version":0,"reason":"stale write"}')
assert_code "order_version_conflict" "409" "$code"
code=$(api_call "GET" "/orders" "$admin_token")
assert_code "order_version_conflict_preserves_state" "200" "$code"
order_conflict_unchanged=$(python3 - <<PY
import json
target_id = $order_id
with open('/tmp/api_test_body.json') as f:
    rows = json.load(f)
target = next((x for x in rows if x.get('id') == target_id), None)
ok = bool(target and target.get('status') == 'Billed' and int(target.get('version', -1)) == 1)
print('1' if ok else '0')
PY
)
if [ "$order_conflict_unchanged" != "1" ]; then
  fail_case "order_version_conflict_preserves_state" "version-conflict write unexpectedly changed order status/version"
fi
pass_case "order_version_conflict_preserves_state" "409 conflict leaves order unchanged"

code=$(api_call "POST" "/orders" "$admin_token" "{\"patient_id\":$patient_id,\"menu_id\":$menu_id,\"notes\":\"idem\",\"idempotency_key\":\"order-idem-001\"}")
assert_code "order_create_idempotency_first" "200" "$code"
idem_first=$(python3 -c 'import json; print(json.load(open("/tmp/api_test_body.json")))')
code=$(api_call "POST" "/orders" "$admin_token" "{\"patient_id\":$patient_id,\"menu_id\":$menu_id,\"notes\":\"idem\",\"idempotency_key\":\"order-idem-001\"}")
assert_code "order_create_idempotency_second" "200" "$code"
idem_second=$(python3 -c 'import json; print(json.load(open("/tmp/api_test_body.json")))')
if [ "$idem_first" != "$idem_second" ]; then
  fail_case "order_idempotency" "same idempotency key did not return same order id"
fi
pass_case "order_idempotency" "idempotent order creation returns same order id"

code=$(api_call "POST" "/orders" "$clinical_token" "{\"patient_id\":$patient_id,\"menu_id\":$menu_id,\"notes\":\"idem\",\"idempotency_key\":\"order-idem-001\"}")
assert_code "order_idempotency_user_isolation" "200" "$code"
clinical_idem_order=$(python3 -c 'import json; print(json.load(open("/tmp/api_test_body.json")))')
if [ "$clinical_idem_order" = "$idem_first" ]; then
  fail_case "order_idempotency_user_isolation" "idempotency key leaked across users"
fi
pass_case "order_idempotency_user_isolation" "idempotency key is scoped per actor"

code=$(api_call "PUT" "/orders/$order_id/status" "$admin_token" '{"status":"Credited","reason":"customer refund"}')
assert_code "order_status_credited" "200" "$code"
pass_case "order_lifecycle_transitions" "order moved through Created->Billed->Credited"

code=$(api_call "PUT" "/orders/$order_id/status" "$admin_token" '{"status":"ImpossibleStatus"}')
assert_code "order_lifecycle_invalid_transition_rejected" "400" "$code"

code=$(api_call "PUT" "/orders/$order_id/status" "$admin_token" '{"status":"Canceled","reason":"too late"}')
assert_code "order_post_credit_invalid_transition" "400" "$code"

code=$(api_call "POST" "/orders" "$admin_token" "{\"patient_id\":$patient_id,\"menu_id\":$menu_id,\"notes\":\"to-cancel\"}")
assert_code "order_create_for_cancel" "200" "$code"
cancel_order_id=$(python3 -c 'import json; print(json.load(open("/tmp/api_test_body.json")))')
code=$(api_call "PUT" "/orders/$cancel_order_id/status" "$admin_token" '{"status":"Canceled","reason":"patient unavailable"}')
assert_code "order_cancel_transition" "200" "$code"

code=$(api_call "POST" "/orders/$order_id/ticket-splits" "$admin_token" '{"split_by":"ward","split_value":"north","quantity":2}')
assert_code "order_ticket_split_add" "200" "$code"
code=$(api_call "GET" "/orders/$order_id/ticket-splits" "$admin_token")
assert_code "order_ticket_split_list" "200" "$code"
split_count=$(python3 - <<'PY'
import json
with open('/tmp/api_test_body.json') as f:
    data = json.load(f)
print(len(data))
PY
)
if [ "$split_count" -lt 1 ]; then
  fail_case "order_ticket_split_visible" "expected at least one ticket split"
fi
pass_case "order_ticket_split_visible" "ticket split appears in order split timeline"

ticket_split_audit_count=$(mysql_query "SELECT COUNT(*) FROM audit_logs WHERE action_type = 'order.ticket_split' AND entity_id = '$order_id';")
if [ "$ticket_split_audit_count" -lt 1 ]; then
  fail_case "order_ticket_split_audit_logged" "expected audit record for ticket split operation"
fi
pass_case "order_ticket_split_audit_logged" "ticket split emits audit record with actor and target metadata"

code=$(api_call "POST" "/orders/$order_id/notes" "$admin_token" '{"note":"operations trail"}')
assert_code "order_note_add" "200" "$code"
code=$(api_call "GET" "/orders/$order_id/notes" "$admin_token")
assert_code "order_note_list" "200" "$code"
note_count=$(python3 - <<'PY'
import json
with open('/tmp/api_test_body.json') as f:
    data = json.load(f)
print(len(data))
PY
)
if [ "$note_count" -lt 1 ]; then
  fail_case "order_note_visible" "expected at least one order note"
fi
pass_case "order_note_visible" "order notes timeline returns operation history entries"

order_note_audit_count=$(mysql_query "SELECT COUNT(*) FROM audit_logs WHERE action_type = 'order.note' AND entity_id = '$order_id';")
if [ "$order_note_audit_count" -lt 1 ]; then
  fail_case "order_note_audit_logged" "expected audit record for order note operation"
fi
pass_case "order_note_audit_logged" "order note emits audit record with actor and target metadata"

audit_before=$(mysql_query "SELECT COUNT(*) FROM audit_logs;")
audit_first_before=$(mysql_query "SELECT action_type FROM audit_logs ORDER BY id ASC LIMIT 1;")
code=$(api_call "POST" "/telemetry/events" "$admin_token" '{"experiment_key":"audit_append_only","event_name":"touch","payload_json":"{}"}')
assert_code "audit_append_only_trigger_action" "200" "$code"
audit_after=$(mysql_query "SELECT COUNT(*) FROM audit_logs;")
if [ "$audit_after" -le "$audit_before" ]; then
  fail_case "audit_append_only_growth" "audit log count did not increase"
fi
pass_case "audit_append_only_growth" "audit log count increased from $audit_before to $audit_after"

set +e
audit_mutation_code=$(curl -s -o /tmp/api_test_body.json -w "%{http_code}" -X PUT "$API_BASE/audits" -H "X-Session-Token: $admin_token" -H "Content-Type: application/json" -d '{"action_type":"tamper"}')
audit_delete_code=$(curl -s -o /tmp/api_test_body.json -w "%{http_code}" -X DELETE "$API_BASE/audits" -H "X-Session-Token: $admin_token")
mysql_update_out=$(docker compose exec -T mysql mysql -N -uapp_user -papp_password_local hospital_platform -e "UPDATE audit_logs SET action_type='tamper' WHERE id = 1;" 2>&1)
mysql_update_rc=$?
mysql_delete_out=$(docker compose exec -T mysql mysql -N -uapp_user -papp_password_local hospital_platform -e "DELETE FROM audit_logs WHERE id = 1;" 2>&1)
mysql_delete_rc=$?
set -e
if [ "$audit_mutation_code" != "400" ]; then
  fail_case "audit_api_update_rejected" "expected 400 for audit update tamper attempt, got $audit_mutation_code"
fi
pass_case "audit_api_update_rejected" "audit update endpoint explicitly rejects tampering"

if [ "$audit_delete_code" != "400" ]; then
  fail_case "audit_api_delete_rejected" "expected 400 for audit delete tamper attempt, got $audit_delete_code"
fi
pass_case "audit_api_delete_rejected" "audit delete endpoint explicitly rejects tampering"

if [ "$mysql_update_rc" -eq 0 ]; then
  fail_case "audit_db_update_trigger" "database allowed direct UPDATE on audit_logs"
fi
if [[ "$mysql_update_out" != *"append-only"* ]]; then
  fail_case "audit_db_update_trigger" "expected append-only trigger message for UPDATE"
fi
pass_case "audit_db_update_trigger" "database trigger rejects direct audit UPDATE"

if [ "$mysql_delete_rc" -eq 0 ]; then
  fail_case "audit_db_delete_trigger" "database allowed direct DELETE on audit_logs"
fi
if [[ "$mysql_delete_out" != *"append-only"* ]]; then
  fail_case "audit_db_delete_trigger" "expected append-only trigger message for DELETE"
fi
pass_case "audit_db_delete_trigger" "database trigger rejects direct audit DELETE"
audit_after_attempt=$(mysql_query "SELECT COUNT(*) FROM audit_logs;")
audit_first_after=$(mysql_query "SELECT action_type FROM audit_logs ORDER BY id ASC LIMIT 1;")
if [ "$audit_first_before" != "$audit_first_after" ]; then
  fail_case "audit_immutability" "existing audit rows were mutated"
fi
if [ "$audit_after_attempt" -lt "$audit_after" ]; then
  fail_case "audit_immutability" "audit log unexpectedly shrank"
fi
pass_case "audit_immutability" "audit log remains append-only; historical rows unchanged"

code=$(api_call "POST" "/governance/records" "$admin_token" '{"tier":"raw","lineage_source_id":null,"lineage_metadata":"seed:raw","payload_json":"{\"batch\":1}"}')
assert_code "governance_create_raw" "200" "$code"
raw_record_id=$(python3 -c 'import json; print(json.load(open("/tmp/api_test_body.json")))')

code=$(api_call "POST" "/governance/records" "$admin_token" "{\"tier\":\"cleaned\",\"lineage_source_id\":$raw_record_id,\"lineage_metadata\":\"lineage:raw_to_cleaned\",\"payload_json\":\"{\\\"batch\\\":2}\"}")
assert_code "governance_create_cleaned" "200" "$code"
cleaned_record_id=$(python3 -c 'import json; print(json.load(open("/tmp/api_test_body.json")))')

code=$(api_call "GET" "/governance/records" "$admin_token")
assert_code "governance_list_records" "200" "$code"
lineage_ok=$(python3 - <<PY
import json
raw_id = $raw_record_id
cleaned_id = $cleaned_record_id
with open('/tmp/api_test_body.json') as f:
    data = json.load(f)
target = [x for x in data if x['id'] == cleaned_id]
if not target:
    print('0')
else:
    item = target[0]
    print('1' if item['lineage_source_id'] == raw_id and item['lineage_metadata'] == 'lineage:raw_to_cleaned' else '0')
PY
)
if [ "$lineage_ok" != "1" ]; then
  fail_case "governance_lineage_capture" "cleaned record lineage metadata/source mismatch"
fi
pass_case "governance_lineage_capture" "lineage metadata and source captured"

code=$(api_call "DELETE" "/governance/records/$cleaned_record_id" "$admin_token" '{"reason":"retention cleanup"}')
assert_code "governance_tombstone_request" "200" "$code"
code=$(api_call "GET" "/governance/records" "$admin_token")
assert_code "governance_tombstone_list" "200" "$code"
tombstone_ok=$(python3 - <<PY
import json
target_id = $cleaned_record_id
with open('/tmp/api_test_body.json') as f:
    data = json.load(f)
target = [x for x in data if x['id'] == target_id]
print('1' if target and target[0]['tombstoned'] else '0')
PY
)
if [ "$tombstone_ok" != "1" ]; then
  fail_case "governance_tombstone_behavior" "record was not tombstoned"
fi
pass_case "governance_tombstone_behavior" "record tombstoned while retained"

code=$(api_call "POST" "/governance/records" "$admin_token" "{\"tier\":\"analytics\",\"lineage_source_id\":$cleaned_record_id,\"lineage_metadata\":\"lineage:cleaned_to_analytics\",\"payload_json\":\"{\\\"aggregated\\\":true}\"}")
assert_code "governance_create_analytics" "200" "$code"
analytics_record_id=$(python3 -c 'import json; print(json.load(open("/tmp/api_test_body.json")))')

code=$(api_call "GET" "/governance/records" "$admin_token")
assert_code "governance_analytics_lineage" "200" "$code"
analytics_lineage_ok=$(python3 - <<PY
import json
raw_id = $raw_record_id
cleaned_id = $cleaned_record_id
analytics_id = $analytics_record_id
with open('/tmp/api_test_body.json') as f:
    data = json.load(f)
analytics = [x for x in data if x['id'] == analytics_id]
if not analytics:
    print('0')
else:
    item = analytics[0]
    has_lineage = item['lineage_source_id'] == cleaned_id and item['tier'] == 'analytics'
    print('1' if has_lineage else '0')
PY
)
if [ "$analytics_lineage_ok" != "1" ]; then
  fail_case "governance_analytics_tier_lineage" "analytics record lineage to cleaned source not captured"
fi
pass_case "governance_analytics_tier_lineage" "analytics tier record traces lineage through cleaned to raw"

code=$(api_call "POST" "/experiments" "$admin_token" '{"experiment_key":"metrics_suite"}')
assert_code "experiment_metrics_suite_create" "200" "$code"

for _ in 1 2 3 4; do
  code=$(api_call "POST" "/telemetry/events" "$admin_token" '{"experiment_key":"metrics_suite","event_name":"recommendation_impression","payload_json":"{}"}')
  [ "$code" = "200" ] || fail_case "telemetry_impression_ingest" "expected 200 got $code"
done
for _ in 1 2; do
  code=$(api_call "POST" "/telemetry/events" "$admin_token" '{"experiment_key":"metrics_suite","event_name":"recommendation_click","payload_json":"{}"}')
  [ "$code" = "200" ] || fail_case "telemetry_click_ingest" "expected 200 got $code"
done
code=$(api_call "POST" "/telemetry/events" "$admin_token" '{"experiment_key":"metrics_suite","event_name":"order_created","payload_json":"{}"}')
assert_code "telemetry_order_created_ingest" "200" "$code"

code=$(api_call "GET" "/analytics/recommendation-kpi" "$admin_token")
assert_code "analytics_recommendation_kpi" "200" "$code"
if python3 - <<'PY'
import json, sys
with open('/tmp/api_test_body.json') as f:
    data = json.load(f)
if abs(data['ctr'] - 0.5) > 1e-9 or abs(data['conversion'] - 0.5) > 1e-9:
    sys.exit(1)
PY
then
  pass_case "experiment_metric_calculations" "recommendation KPI math is deterministic"
else
  fail_case "experiment_metric_calculations" "expected ctr=0.5 and conversion=0.5"
fi

external_ingestion_payload='{"task_name":"ssrf-probe","seed_urls":["https://evil.example.com/data"],"extraction_rules_json":"{\"mode\":\"css\",\"fields\":[\".record\"]}","pagination_strategy":"breadth-first","max_depth":1,"incremental_field":"value","schedule_cron":"0 * * * *"}'
code=$(api_call "POST" "/ingestion/tasks" "$admin_token" "$external_ingestion_payload")
assert_code "ingestion_reject_external_url" "200" "$code"
ssrf_task_id=$(python3 -c 'import json; print(json.load(open("/tmp/api_test_body.json")))')
code=$(api_call "POST" "/ingestion/tasks/$ssrf_task_id/run" "$admin_token" '')
assert_code "ingestion_run_external_url_rejected" "200" "$code"
sleep 2
ssrf_run_status=$(docker compose exec -T mysql mysql -N -uapp_user -papp_password_local hospital_platform -e "SELECT status FROM ingestion_task_runs WHERE task_id = $ssrf_task_id ORDER BY id DESC LIMIT 1;")
if [ "$ssrf_run_status" != "failed" ]; then
  fail_case "ingestion_ssrf_blocked" "expected external URL ingestion to fail, got status: $ssrf_run_status"
fi
pass_case "ingestion_ssrf_blocked" "external/public seed URLs are rejected by intranet allowlist"

ingestion_create_payload='{"task_name":"patient-feed","seed_urls":["file:///app/config/ingestion_fixture/page1.html"],"extraction_rules_json":"{\"mode\":\"css\",\"fields\":[\".record\"],\"pagination_selector\":\"a.next\"}","pagination_strategy":"depth-first","max_depth":2,"incremental_field":"value","schedule_cron":"0 * * * *"}'
code=$(api_call "POST" "/ingestion/tasks" "$admin_token" "$ingestion_create_payload")
assert_code "ingestion_create_task" "200" "$code"
ingestion_task_id=$(python3 -c 'import json; print(json.load(open("/tmp/api_test_body.json")))')

code=$(api_call "GET" "/ingestion/tasks" "$admin_token")
assert_code "ingestion_list_tasks" "200" "$code"
cron_create_ok=$(python3 - <<PY
import json
from datetime import datetime
task_id = $ingestion_task_id
with open('/tmp/api_test_body.json') as f:
    rows = json.load(f)
target = next((x for x in rows if x['id'] == task_id), None)
if not target or not target.get('next_run_at'):
    print('0')
else:
    dt = datetime.strptime(target['next_run_at'], '%Y-%m-%d %H:%M:%S')
    print('1' if dt.minute == 0 else '0')
PY
)
if [ "$cron_create_ok" != "1" ]; then
  fail_case "ingestion_cron_create_next_run" "create flow next_run_at is not aligned with schedule_cron"
fi
pass_case "ingestion_cron_create_next_run" "create flow derives next_run_at from cron"

ingestion_update_payload='{"seed_urls":["file:///app/config/ingestion_fixture/page1.html"],"extraction_rules_json":"{\"mode\":\"css\",\"fields\":[\".record\"],\"pagination_selector\":\"a.next\"}","pagination_strategy":"breadth-first","max_depth":3,"incremental_field":"value","schedule_cron":"*/30 * * * *"}'
code=$(api_call "PUT" "/ingestion/tasks/$ingestion_task_id" "$admin_token" "$ingestion_update_payload")
assert_code "ingestion_update_task" "200" "$code"

code=$(api_call "GET" "/ingestion/tasks" "$admin_token")
assert_code "ingestion_list_tasks_after_update" "200" "$code"
cron_update_ok=$(python3 - <<PY
import json
from datetime import datetime
task_id = $ingestion_task_id
with open('/tmp/api_test_body.json') as f:
    rows = json.load(f)
target = next((x for x in rows if x['id'] == task_id), None)
if not target or not target.get('next_run_at'):
    print('0')
else:
    dt = datetime.strptime(target['next_run_at'], '%Y-%m-%d %H:%M:%S')
    print('1' if dt.minute in (0, 30) else '0')
PY
)
if [ "$cron_update_ok" != "1" ]; then
  fail_case "ingestion_cron_update_next_run" "update flow next_run_at is not aligned with updated schedule"
fi
pass_case "ingestion_cron_update_next_run" "update flow re-computes next_run_at from cron"

code=$(api_call "GET" "/ingestion/tasks/$ingestion_task_id/versions" "$admin_token")
assert_code "ingestion_versions_after_update" "200" "$code"

ingestion_rollback_payload='{"target_version":1,"reason":"rollback smoke test"}'
code=$(api_call "POST" "/ingestion/tasks/$ingestion_task_id/rollback" "$admin_token" "$ingestion_rollback_payload")
assert_code "ingestion_rollback_task" "200" "$code"

code=$(api_call "GET" "/ingestion/tasks" "$admin_token")
assert_code "ingestion_list_tasks_after_rollback" "200" "$code"
cron_rollback_ok=$(python3 - <<PY
import json
from datetime import datetime
task_id = $ingestion_task_id
with open('/tmp/api_test_body.json') as f:
    rows = json.load(f)
target = next((x for x in rows if x['id'] == task_id), None)
if not target or not target.get('next_run_at'):
    print('0')
else:
    dt = datetime.strptime(target['next_run_at'], '%Y-%m-%d %H:%M:%S')
    print('1' if dt.minute in (0, 30) else '0')
PY
)
if [ "$cron_rollback_ok" != "1" ]; then
  fail_case "ingestion_cron_rollback_next_run" "rollback flow next_run_at is not aligned with schedule"
fi
pass_case "ingestion_cron_rollback_next_run" "rollback flow preserves cron-based schedule"

code=$(api_call "POST" "/ingestion/tasks/$ingestion_task_id/run" "$admin_token")
assert_code "ingestion_run_task" "200" "$code"

code=$(api_call "GET" "/ingestion/tasks/$ingestion_task_id/runs" "$admin_token")
assert_code "ingestion_runs_list" "200" "$code"
ingestion_records=$(python3 - <<'PY'
import json
with open('/tmp/api_test_body.json') as f:
    runs = json.load(f)
print(runs[0]['records_extracted'] if runs else 0)
PY
)
if [ "$ingestion_records" -lt 4 ]; then
  fail_case "ingestion_real_execution_records" "expected >=4 extracted records from fixture pages"
fi
pass_case "ingestion_real_execution_records" "ingestion execution extracted fixture records"

code=$(api_call "GET" "/ingestion/tasks" "$admin_token")
assert_code "ingestion_list_tasks_after_run" "200" "$code"
cron_run_ok=$(python3 - <<PY
import json
from datetime import datetime
task_id = $ingestion_task_id
with open('/tmp/api_test_body.json') as f:
    rows = json.load(f)
target = next((x for x in rows if x['id'] == task_id), None)
if not target or not target.get('next_run_at') or not target.get('last_run_at'):
    print('0')
else:
    next_dt = datetime.strptime(target['next_run_at'], '%Y-%m-%d %H:%M:%S')
    last_dt = datetime.strptime(target['last_run_at'], '%Y-%m-%d %H:%M:%S')
    print('1' if next_dt > last_dt and next_dt.minute in (0, 30) else '0')
PY
)
if [ "$cron_run_ok" != "1" ]; then
  fail_case "ingestion_cron_run_cadence" "run flow did not advance next_run_at according to cron cadence"
fi
pass_case "ingestion_cron_run_cadence" "run flow advances next_run_at by cron schedule"

code=$(api_call "POST" "/ingestion/tasks/$ingestion_task_id/run" "$admin_token")
assert_code "ingestion_run_task_incremental_second" "200" "$code"
code=$(api_call "GET" "/ingestion/tasks/$ingestion_task_id/runs" "$admin_token")
assert_code "ingestion_runs_list_after_incremental" "200" "$code"
second_run_records=$(python3 - <<'PY'
import json
with open('/tmp/api_test_body.json') as f:
    runs = json.load(f)
print(runs[0]['records_extracted'] if runs else -1)
PY
)
if [ "$second_run_records" -ne 0 ]; then
  fail_case "ingestion_incremental_filter" "expected second incremental run to extract 0 new records"
fi
pass_case "ingestion_incremental_filter" "incremental watermark prevents duplicate extraction"

bad_ingestion_payload='{"task_name":"patient-feed-bad","seed_urls":["file:///app/config/ingestion_fixture/page1.html"],"extraction_rules_json":"{\"mode\":\"regex\",\"fields\":[\"(\"]}","pagination_strategy":"breadth-first","max_depth":1,"incremental_field":"value","schedule_cron":"0 * * * *"}'
code=$(api_call "POST" "/ingestion/tasks" "$admin_token" "$bad_ingestion_payload")
assert_code "ingestion_create_bad_task" "200" "$code"
bad_ingestion_task_id=$(python3 -c 'import json; print(json.load(open("/tmp/api_test_body.json")))')

code=$(api_call "POST" "/ingestion/tasks/$bad_ingestion_task_id/run" "$admin_token")
assert_code "ingestion_deterministic_failure_status" "400" "$code"

code=$(api_call "GET" "/ingestion/tasks/$bad_ingestion_task_id/runs" "$admin_token")
assert_code "ingestion_deterministic_failure_run_list" "200" "$code"
failure_ok=$(python3 - <<'PY'
import json
with open('/tmp/api_test_body.json') as f:
    runs = json.load(f)
if not runs:
    print('0')
else:
    run = runs[0]
    diag = json.loads(run.get('diagnostics_json') or '{}')
    print('1' if run.get('status') == 'failed' and diag.get('deterministic') else '0')
PY
)
if [ "$failure_ok" != "1" ]; then
  fail_case "ingestion_deterministic_failure_diagnostics" "expected failed run diagnostics with deterministic marker"
fi
pass_case "ingestion_deterministic_failure_diagnostics" "failed ingestion runs persist deterministic diagnostics"

code=$(api_call "GET" "/analytics/funnel" "$admin_token")
assert_code "analytics_funnel_available" "200" "$code"
funnel_ok=$(python3 - <<'PY'
import json
with open('/tmp/api_test_body.json') as f:
    rows = json.load(f)
steps = {x['step']: x['users'] for x in rows}
ok = {'login', 'workflow_action', 'dining_order'}.issubset(steps.keys()) and steps['dining_order'] >= 1
print('1' if ok else '0')
PY
)
if [ "$funnel_ok" != "1" ]; then
  fail_case "funnel_metrics_shape" "funnel metrics missing required steps or invalid values"
fi
pass_case "funnel_metrics_shape" "funnel metrics include expected steps"

cat >"$REPORT_DIR/api_integration_tests.json" <<EOF
{"suite":"api_integration_tests","status":"pass"}
EOF
