// @ts-check
const { test, expect } = require('@playwright/test');

// Cafeteria-role E2E flow:
//   1. Load the SPA and log in as cafeteria1
//   2. Assert cafeteria-permitted nav items are visible
//   3. Assert patient and admin nav items are NOT visible (role isolation)
//   4. Navigate to the Cafeteria Manager and verify the panel loads

test.describe('Cafeteria user — role isolation and dining section access', () => {
  test('cafeteria1 sees cafeteria nav, blocked from patient and admin sections', async ({ page }) => {

    // ── 1. Load the SPA ──────────────────────────────────────────────
    await page.goto('/');
    await expect(page.locator('section.login-card')).toBeVisible({ timeout: 20_000 });

    // ── 2. Log in as cafeteria1 ──────────────────────────────────────
    await page.locator('input[placeholder="admin"]').fill('cafeteria1');
    await page.locator('input[type="password"]').fill('Admin#OfflinePass123');
    await page.locator('button', { hasText: 'Sign In' }).click();

    // ── 3. Wait for shell / sidebar ──────────────────────────────────
    await expect(page.locator('aside.sidebar')).toBeVisible({ timeout: 25_000 });
    await expect(page.locator('aside.sidebar p.muted')).toContainText('cafeteria1');

    // ── 4. Cafeteria nav items must be accessible ────────────────────
    await expect(
      page.locator('aside.sidebar button.nav', { hasText: 'Cafeteria' })
    ).toBeVisible({ timeout: 5_000 });

    // ── 5. Patient and admin nav items must be absent (data isolation) ─
    // These sections are hidden by the WASM entitlement check; their absence
    // in the DOM is the UI-level enforcement of role-based data isolation.
    await expect(
      page.locator('aside.sidebar button.nav', { hasText: 'Patients' })
    ).not.toBeVisible({ timeout: 5_000 });

    await expect(
      page.locator('aside.sidebar button.nav', { hasText: 'Admin' })
    ).not.toBeVisible({ timeout: 5_000 });

    await expect(
      page.locator('aside.sidebar button.nav', { hasText: 'Experiments' })
    ).not.toBeVisible({ timeout: 5_000 });

    await expect(
      page.locator('aside.sidebar button.nav', { hasText: 'Analytics' })
    ).not.toBeVisible({ timeout: 5_000 });

    // ── 6. Navigate to Cafeteria and verify the panel loads ──────────
    await page.locator('aside.sidebar button.nav', { hasText: 'Cafeteria' }).click();
    await expect(
      page.locator('article.panel h3', { hasText: 'Cafeteria Manager' })
    ).toBeVisible({ timeout: 10_000 });

    // Click Refresh Dining Data and verify the API call succeeds
    const [diningResp] = await Promise.all([
      page.waitForResponse(
        r => r.url().includes('/cafeteria/categories') && r.status() === 200,
        { timeout: 15_000 }
      ),
      page.locator('button.primary', { hasText: 'Refresh Dining Data' }).click(),
    ]);

    // Cafeteria categories endpoint must return 200 for this role
    expect(diningResp.status()).toBe(200);

    // ── 7. No error banners ──────────────────────────────────────────
    await expect(page.locator('p.error')).not.toBeVisible();
  });
});
