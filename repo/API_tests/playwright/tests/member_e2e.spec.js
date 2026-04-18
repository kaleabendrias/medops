// @ts-check
const { test, expect } = require('@playwright/test');

// Member-role E2E flow:
//   1. Load the SPA and log in as member1
//   2. Assert restricted nav items are NOT visible in the sidebar
//   3. Verify the session shows the member username and that sign-out works

test.describe('Member user — restricted navigation and sign-out', () => {
  test('member1 has limited sidebar and cannot see clinical or admin sections', async ({ page }) => {

    // ── 1. Load the SPA ──────────────────────────────────────────────
    await page.goto('/');
    await expect(page.locator('section.login-card')).toBeVisible({ timeout: 20_000 });

    // ── 2. Log in as member1 ─────────────────────────────────────────
    await page.locator('input[placeholder="admin"]').fill('member1');
    await page.locator('input[type="password"]').fill('Admin#OfflinePass123');
    await page.locator('button', { hasText: 'Sign In' }).click();

    // ── 3. Wait for shell / sidebar ──────────────────────────────────
    await expect(page.locator('aside.sidebar')).toBeVisible({ timeout: 25_000 });
    await expect(page.locator('aside.sidebar p.muted')).toContainText('member1');

    // ── 4. Restricted sections must be absent ────────────────────────
    // The WASM frontend filters nav items by entitlement — absence of these
    // buttons is the DOM-level proof that role-based access is enforced.
    const restrictedSections = ['Patients', 'Admin', 'Experiments', 'Analytics', 'Ingestion'];
    for (const label of restrictedSections) {
      await expect(
        page.locator('aside.sidebar button.nav', { hasText: label })
      ).not.toBeVisible({ timeout: 5_000 });
    }

    // ── 5. Dashboard must be accessible ─────────────────────────────
    // Dashboard is always reachable regardless of role — the sidebar must
    // show it so the member can navigate to their permitted landing page.
    await expect(
      page.locator('aside.sidebar button.nav', { hasText: 'Dashboard' })
    ).toBeVisible({ timeout: 5_000 });

    // ── 6. Sign Out returns to the login card ────────────────────────
    await page.locator('aside.sidebar button.danger', { hasText: 'Sign Out' }).click();
    await expect(page.locator('section.login-card')).toBeVisible({ timeout: 15_000 });

    // After sign-out the sidebar must be gone
    await expect(page.locator('aside.sidebar')).not.toBeVisible({ timeout: 5_000 });

    // ── 7. No error banners at any point ─────────────────────────────
    await expect(page.locator('p.error')).not.toBeVisible();
  });
});
