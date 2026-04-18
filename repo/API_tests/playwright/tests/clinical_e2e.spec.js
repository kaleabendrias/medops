// @ts-check
const { test, expect } = require('@playwright/test');

// Clinical-role E2E flow:
//   1. Load the SPA
//   2. Log in as clinical1 (Clinical role — patient management + sensitive reveal)
//   3. Navigate to the Patients section via the sidebar
//   4. Perform a patient search and verify the DOM state change that results
//      from the WASM app processing the API response

test.describe('Clinical user — patient search flow', () => {
  test('clinical1 logs in, opens Patients, searches, and DOM reflects API response', async ({ page }) => {

    // ── 1. Load the SPA ──────────────────────────────────────────────
    await page.goto('/');

    // The login card should be visible immediately
    await expect(page.locator('section.login-card')).toBeVisible({ timeout: 20_000 });

    // ── 2. Log in as clinical1 ───────────────────────────────────────
    await page.locator('input[placeholder="admin"]').fill('clinical1');
    await page.locator('input[type="password"]').fill('Admin#OfflinePass123');
    await page.locator('button', { hasText: 'Sign In' }).click();

    // ── 3. Wait for shell / sidebar (login succeeded + WASM re-rendered) ──
    await expect(page.locator('aside.sidebar')).toBeVisible({ timeout: 25_000 });

    // Sidebar should display the username and role
    await expect(page.locator('aside.sidebar p.muted')).toContainText('clinical1');

    // ── 4. Navigate to the Patients section ──────────────────────────
    await page.locator('aside.sidebar button.nav', { hasText: 'Patients' }).click();

    await expect(
      page.locator('article.panel h3', { hasText: 'Patient Workspace' })
    ).toBeVisible({ timeout: 10_000 });

    // ── 5. Fill the search input ─────────────────────────────────────
    const searchInput = page.locator('input[placeholder="Search by MRN or name"]');
    await expect(searchInput).toBeVisible();
    await searchInput.fill('john');

    // ── 6. Click Search and intercept the API response ───────────────
    // Promise.all ensures we start listening before the click fires the request.
    const [searchResp] = await Promise.all([
      page.waitForResponse(
        r => r.url().includes('/patients/search') && r.status() === 200,
        { timeout: 15_000 }
      ),
      page.locator('button.primary', { hasText: 'Search' }).click(),
    ]);

    // ── 7. Verify the DOM state change ───────────────────────────────
    // If the API returned results, verify the cards rendered correctly.
    const apiBody = await searchResp.json();
    if (Array.isArray(apiBody) && apiBody.length > 0) {
      await expect(page.locator('div.cards button.card.left').first()).toBeVisible({ timeout: 10_000 });
      await expect(page.locator('div.cards button.card.left strong').first()).not.toBeEmpty();
      await expect(page.locator('div.cards button.card.left p.muted').first()).toContainText('MRN:');
    }

    // ── 8. Verify no error banner was shown ──────────────────────────
    await expect(page.locator('.error-banner, [class*="error"]')).not.toBeVisible();
  });
});
