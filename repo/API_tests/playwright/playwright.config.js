// @ts-check
const { defineConfig, devices } = require('@playwright/test');

module.exports = defineConfig({
  testDir: './tests',
  timeout: 90_000,
  expect: { timeout: 15_000 },
  use: {
    baseURL: 'https://web:8443',
    ignoreHTTPSErrors: true,
    headless: true,
    actionTimeout: 15_000,
    navigationTimeout: 30_000,
  },
  reporter: [
    ['json', { outputFile: '/workspace/test_reports/playwright_e2e.json' }],
    ['list'],
  ],
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
});
