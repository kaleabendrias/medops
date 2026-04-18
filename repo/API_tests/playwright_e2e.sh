#!/usr/bin/env bash
# Runs the Playwright browser E2E suite inside the playwright_tests container.
# Executes real DOM interactions against the running web service (https://web:8443).
set -uo pipefail

REPORT_DIR="${1:-test_reports}"
mkdir -p "/workspace/$REPORT_DIR"

cd /workspace/API_tests/playwright

# Install local node_modules so require('@playwright/test') resolves correctly.
npm install --prefer-offline 2>/dev/null || npm install

# playwright test writes the JSON report to the path configured in
# playwright.config.js (REPORT_DIR/playwright_e2e.json).
# The process exit code is non-zero on any test failure.
npx playwright test \
  --config=/workspace/API_tests/playwright/playwright.config.js
