#!/usr/bin/env bash
set -euo pipefail

REPORT_DIR="${1:-test_reports}"
mkdir -p "$REPORT_DIR"

CASE_FILE="$REPORT_DIR/migration_checks.ndjson"
: >"$CASE_FILE"

record_case() {
  local name="$1"
  local status="$2"
  local detail="$3"
  printf '{"suite":"migration_checks","case":"%s","status":"%s","detail":"%s"}\n' "$name" "$status" "$detail" >>"$CASE_FILE"
}

fail_case() {
  record_case "$1" "fail" "$2"
  cat >"$REPORT_DIR/migration_checks.json" <<EOF
{"suite":"migration_checks","status":"fail","failed_case":"$1"}
EOF
  exit 1
}

pass_case() {
  record_case "$1" "pass" "$2"
}

mysql_query() {
  docker compose exec -T mysql mysql -N -uapp_user -papp_password_local hospital_platform -e "$1"
}

expected_migrations=$(ls -1 services/api/migrations/*.sql 2>/dev/null | wc -l)
migration_count=$(mysql_query "SELECT COUNT(*) FROM _sqlx_migrations WHERE success = 1;")
if [ "$migration_count" -ne "$expected_migrations" ]; then
  fail_case "migrations_applied" "expected exactly $expected_migrations successful migrations, got $migration_count"
fi
pass_case "migrations_applied" "all $migration_count of $expected_migrations migrations applied successfully"

latest_migration_file=$(ls -1 services/api/migrations/*.sql | sort | tail -1 | xargs basename)
latest_version_applied=$(mysql_query "SELECT MAX(version) FROM _sqlx_migrations WHERE success = 1;")
expected_version=$(echo "$latest_migration_file" | grep -oP '^\d+' | sed 's/^0*//')
latest_version_applied=$(echo "$latest_version_applied" | tr -d '[:space:]')
if [ "$latest_version_applied" != "$expected_version" ]; then
  fail_case "latest_migration_version" "expected latest migration version $expected_version, got $latest_version_applied"
fi
pass_case "latest_migration_version" "latest migration version is $latest_version_applied"

password_hash_len=$(mysql_query "SELECT CHARACTER_MAXIMUM_LENGTH FROM information_schema.columns WHERE table_schema='hospital_platform' AND table_name='users' AND column_name='password_hash';")
if [ -z "$password_hash_len" ] || [ "$password_hash_len" -lt 255 ]; then
  fail_case "password_hash_column_size" "expected users.password_hash length >= 255, got ${password_hash_len:-missing}"
fi
pass_case "password_hash_column_size" "users.password_hash supports Argon2id payloads"

seed_users=$(mysql_query "SELECT COUNT(*) FROM users WHERE username IN ('admin','employee1','member1','lockout_user','clinical1','cafeteria1');")
if [ "$seed_users" -ne 6 ]; then
  fail_case "seed_users_present" "expected 6 seeded users, got $seed_users"
fi
pass_case "seed_users_present" "seeded users present"

campaign_deadline_col=$(mysql_query "SELECT COUNT(*) FROM information_schema.columns WHERE table_schema='hospital_platform' AND table_name='group_campaigns' AND column_name='success_deadline_at';")
if [ "$campaign_deadline_col" -ne 1 ]; then
  fail_case "campaign_deadline_schema" "success_deadline_at column missing on group_campaigns"
fi
pass_case "campaign_deadline_schema" "campaign deadline column present"

audit_triggers=$(mysql_query "SELECT COUNT(*) FROM information_schema.triggers WHERE trigger_schema='hospital_platform' AND trigger_name IN ('trg_audit_logs_block_update','trg_audit_logs_block_delete');")
if [ "$audit_triggers" -ne 2 ]; then
  fail_case "audit_append_only_triggers" "expected 2 audit immutability triggers, got $audit_triggers"
fi
pass_case "audit_append_only_triggers" "database-level audit immutability triggers present"

retention_floor=$(mysql_query "SELECT years FROM retention_policies WHERE policy_key = 'clinical_records';")
if [ -z "$retention_floor" ] || [ "$retention_floor" -lt 7 ]; then
  fail_case "clinical_retention_floor" "expected clinical_records >= 7, got ${retention_floor:-missing}"
fi
pass_case "clinical_retention_floor" "clinical retention floor is $retention_floor years"

entitlements=$(mysql_query "SELECT COUNT(*) FROM menu_entitlements;")
if [ "$entitlements" -le 0 ]; then
  fail_case "menu_entitlements_seeded" "menu_entitlements table has no rows"
fi
pass_case "menu_entitlements_seeded" "menu entitlements seeded"

ingestion_tables=$(mysql_query "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'hospital_platform' AND table_name IN ('ingestion_tasks','ingestion_task_versions','ingestion_task_runs','ingestion_task_records');")
if [ "$ingestion_tables" -ne 4 ]; then
  fail_case "ingestion_schema_present" "expected ingestion manager tables, got $ingestion_tables"
fi
pass_case "ingestion_schema_present" "ingestion task manager tables are present"

patient_assignment_table=$(mysql_query "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'hospital_platform' AND table_name = 'patient_assignments';")
if [ "$patient_assignment_table" -ne 1 ]; then
  fail_case "patient_assignment_schema_present" "patient_assignments table missing"
fi
pass_case "patient_assignment_schema_present" "patient assignment table present"

attachment_payload_col=$(mysql_query "SELECT COUNT(*) FROM information_schema.columns WHERE table_schema = 'hospital_platform' AND table_name = 'patient_attachments' AND column_name = 'payload_blob';")
if [ "$attachment_payload_col" -ne 1 ]; then
  fail_case "attachment_payload_mysql_authority_schema" "patient_attachments.payload_blob column missing"
fi
pass_case "attachment_payload_mysql_authority_schema" "patient attachment payload column present"

idempotency_index=$(mysql_query "SELECT COUNT(*) FROM information_schema.statistics WHERE table_schema = 'hospital_platform' AND table_name = 'dining_orders' AND index_name = 'uniq_order_idempotency_user_key';")
if [ "$idempotency_index" -lt 2 ]; then
  fail_case "order_idempotency_index_scope" "composite idempotency index missing"
fi
pass_case "order_idempotency_index_scope" "order idempotency index scoped by user"

cat >"$REPORT_DIR/migration_checks.json" <<EOF
{"suite":"migration_checks","status":"pass","cases":12}
EOF
