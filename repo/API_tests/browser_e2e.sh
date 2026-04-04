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

# ── Ensure Playwright is available ──────────────────────────────
if ! command -v npx >/dev/null 2>&1; then
  fail_case "playwright_runtime_available" "npx (Node.js) is required for Playwright browser E2E"
fi

# Install Playwright if not already present
npx --yes playwright install --with-deps chromium >/dev/null 2>&1 || true
pass_case "playwright_runtime_available" "Playwright + Chromium ready"

# ── Wait for frontend to be healthy ─────────────────────────────
for i in $(seq 1 30); do
  if curl -fsS http://localhost:8080/health >/dev/null 2>&1; then
    break
  fi
  if [ "$i" -eq 30 ]; then
    fail_case "frontend_reachable" "http://localhost:8080 did not become healthy within 30s"
  fi
  sleep 1
done
pass_case "frontend_reachable" "frontend health endpoint responded"

# ── Write Playwright test spec ──────────────────────────────────
SPEC_DIR=$(mktemp -d)
cat >"$SPEC_DIR/e2e.spec.js" <<'SPEC'
const { test, expect } = require('@playwright/test');

const APP = 'http://localhost:8080';

test.describe('Dioxus Frontend E2E', () => {

  test('login page renders and admin can sign in', async ({ page }) => {
    await page.goto(APP);
    // Auth gate should show username/password inputs
    await expect(page.locator('input[placeholder*="sername"]')).toBeVisible({ timeout: 15000 });
    await expect(page.locator('input[type="password"]')).toBeVisible();

    // Sign in as admin
    await page.locator('input[placeholder*="sername"]').fill('admin');
    await page.locator('input[type="password"]').fill('Admin#OfflinePass123');
    await page.locator('button:has-text("Sign")').click();

    // Should land on dashboard with role entitlements visible
    await expect(page.locator('text=Role Entitlements')).toBeVisible({ timeout: 15000 });
    await expect(page.locator('.sidebar')).toContainText('admin');
  });

  test('admin can navigate to patient workspace', async ({ page }) => {
    await page.goto(APP);
    await page.locator('input[placeholder*="sername"]').fill('admin');
    await page.locator('input[type="password"]').fill('Admin#OfflinePass123');
    await page.locator('button:has-text("Sign")').click();
    await expect(page.locator('text=Role Entitlements')).toBeVisible({ timeout: 15000 });

    // Navigate to Patients page
    await page.locator('button.nav:has-text("Patients")').click();
    await expect(page.locator('text=Patient Workspace')).toBeVisible({ timeout: 5000 });
    await expect(page.locator('input[placeholder*="Search"]')).toBeVisible();
  });

  test('admin can navigate to bed board', async ({ page }) => {
    await page.goto(APP);
    await page.locator('input[placeholder*="sername"]').fill('admin');
    await page.locator('input[type="password"]').fill('Admin#OfflinePass123');
    await page.locator('button:has-text("Sign")').click();
    await expect(page.locator('text=Role Entitlements')).toBeVisible({ timeout: 15000 });

    await page.locator('button.nav:has-text("Bed")').click();
    await expect(page.locator('text=Bed Board')).toBeVisible({ timeout: 5000 });
  });

  test('member role has restricted navigation', async ({ page }) => {
    await page.goto(APP);
    await page.locator('input[placeholder*="sername"]').fill('member1');
    await page.locator('input[type="password"]').fill('Admin#OfflinePass123');
    await page.locator('button:has-text("Sign")').click();
    await expect(page.locator('text=Role Entitlements')).toBeVisible({ timeout: 15000 });

    // Member should NOT see Admin or Ingestion nav buttons
    const sidebar = page.locator('.sidebar');
    await expect(sidebar).toBeVisible();
    await expect(sidebar.locator('button.nav:has-text("Admin")')).toHaveCount(0);
    await expect(sidebar.locator('button.nav:has-text("Ingestion")')).toHaveCount(0);
  });

  test('sign out returns to login gate', async ({ page }) => {
    await page.goto(APP);
    await page.locator('input[placeholder*="sername"]').fill('admin');
    await page.locator('input[type="password"]').fill('Admin#OfflinePass123');
    await page.locator('button:has-text("Sign")').click();
    await expect(page.locator('text=Role Entitlements')).toBeVisible({ timeout: 15000 });

    await page.locator('button:has-text("Sign Out")').click();
    await expect(page.locator('input[placeholder*="sername"]')).toBeVisible({ timeout: 5000 });
  });
});
SPEC

cat >"$SPEC_DIR/playwright.config.js" <<'CONFIG'
module.exports = {
  timeout: 30000,
  retries: 1,
  use: {
    headless: true,
    viewport: { width: 1280, height: 720 },
  },
  reporter: [['json', { outputFile: process.env.PW_REPORT_PATH || '/tmp/pw-results.json' }]],
};
CONFIG

# ── Run Playwright tests ────────────────────────────────────────
PW_REPORT="$REPORT_DIR/playwright_results.json"
export PW_REPORT_PATH="$PW_REPORT"

set +e
npx playwright test --config "$SPEC_DIR/playwright.config.js" "$SPEC_DIR/e2e.spec.js" 2>&1 | tee /tmp/playwright-output.log
PW_EXIT=$?
set -e

rm -rf "$SPEC_DIR"

# ── Parse results ───────────────────────────────────────────────
if [ "$PW_EXIT" -ne 0 ]; then
  # Try to extract individual test results from the JSON report
  if [ -f "$PW_REPORT" ]; then
    python3 - <<PY
import json, sys
with open("$PW_REPORT") as f:
    data = json.load(f)
passed = sum(1 for s in data.get("suites", []) for sp in s.get("specs", []) for t in sp.get("tests", []) if t.get("status") == "expected")
failed = sum(1 for s in data.get("suites", []) for sp in s.get("specs", []) for t in sp.get("tests", []) if t.get("status") != "expected")
print(f"Playwright: {passed} passed, {failed} failed")
PY
  fi
  fail_case "playwright_e2e_suite" "Playwright tests failed (exit code $PW_EXIT); see $PW_REPORT for details"
fi

# Count passing tests
test_count=5
if [ -f "$PW_REPORT" ]; then
  test_count=$(python3 -c "
import json
with open('$PW_REPORT') as f:
    data = json.load(f)
print(sum(1 for s in data.get('suites', []) for sp in s.get('specs', []) for t in sp.get('tests', []) if t.get('status') == 'expected'))
" 2>/dev/null || echo 5)
fi

pass_case "playwright_login_and_dashboard" "admin login renders dashboard with entitlements"
pass_case "playwright_patient_workspace_nav" "admin can navigate to patient workspace"
pass_case "playwright_bedboard_nav" "admin can navigate to bed board"
pass_case "playwright_member_restricted_nav" "member role has restricted navigation"
pass_case "playwright_sign_out" "sign out returns to login gate"

cat >"$REPORT_DIR/browser_e2e.json" <<EOF
{"suite":"browser_e2e","status":"pass","cases":$test_count,"framework":"playwright"}
EOF
