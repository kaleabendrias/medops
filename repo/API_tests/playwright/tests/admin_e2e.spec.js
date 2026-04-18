// @ts-check
const { test, expect } = require('@playwright/test');

// Admin-role E2E flow:
//   1. Load the SPA and log in as admin
//   2. Assert all admin-gated navigation sections are visible
//   3. Navigate to the Admin console and verify user management DOM state
//   4. Navigate to Patients, search, click a result, and verify
//      that sensitive fields are masked (admin lacks reveal_sensitive)

test.describe('Admin user — full navigation access and patient data masking', () => {
  test('admin sees all gated nav sections and patient fields are masked', async ({ page }) => {

    // ── 1. Load the SPA ──────────────────────────────────────────────
    await page.goto('/');
    await expect(page.locator('section.login-card')).toBeVisible({ timeout: 20_000 });

    // ── 2. Log in as admin ───────────────────────────────────────────
    await page.locator('input[placeholder="admin"]').fill('admin');
    await page.locator('input[type="password"]').fill('Admin#OfflinePass123');
    await page.locator('button', { hasText: 'Sign In' }).click();

    // ── 3. Wait for shell / sidebar ──────────────────────────────────
    await expect(page.locator('aside.sidebar')).toBeVisible({ timeout: 25_000 });
    await expect(page.locator('aside.sidebar p.muted')).toContainText('admin');

    // ── 4. Assert all admin-gated nav items are present ──────────────
    for (const label of ['Patients', 'Admin', 'Experiments', 'Analytics', 'Ingestion']) {
      await expect(
        page.locator('aside.sidebar button.nav', { hasText: label })
      ).toBeVisible({ timeout: 5_000 });
    }

    // ── 5. Admin console — user management DOM ───────────────────────
    await page.locator('aside.sidebar button.nav', { hasText: 'Admin' }).click();
    await expect(
      page.locator('article.panel h3', { hasText: 'Administrator Console' })
    ).toBeVisible({ timeout: 10_000 });

    const [usersResp] = await Promise.all([
      page.waitForResponse(
        r => r.url().includes('/admin/users') && r.status() === 200,
        { timeout: 15_000 }
      ),
      page.locator('button.primary', { hasText: 'Refresh Users' }).click(),
    ]);

    const usersBody = await usersResp.json();
    if (Array.isArray(usersBody) && usersBody.length > 0) {
      // Verify the component rendered user cards in the DOM
      await expect(page.locator('div.cards article.card').first()).toBeVisible({ timeout: 10_000 });
      // Each card must show the username in a <strong> element
      await expect(
        page.locator('div.cards article.card strong').first()
      ).not.toBeEmpty();
      // Each card must show role and disabled status in the muted paragraph
      await expect(
        page.locator('div.cards article.card p.muted').first()
      ).toContainText('role:');
    }

    // ── 6. Patient masking — admin lacks reveal_sensitive ────────────
    await page.locator('aside.sidebar button.nav', { hasText: 'Patients' }).click();
    await expect(
      page.locator('article.panel h3', { hasText: 'Patient Workspace' })
    ).toBeVisible({ timeout: 10_000 });

    const searchInput = page.locator('input[placeholder="Search by MRN or name"]');
    await expect(searchInput).toBeVisible();
    await searchInput.fill('john');

    const [searchResp] = await Promise.all([
      page.waitForResponse(
        r => r.url().includes('/patients/search') && r.status() === 200,
        { timeout: 15_000 }
      ),
      page.locator('button.primary', { hasText: 'Search' }).click(),
    ]);

    const apiBody = await searchResp.json();
    if (Array.isArray(apiBody) && apiBody.length > 0) {
      await expect(page.locator('div.cards button.card.left').first()).toBeVisible({ timeout: 10_000 });

      // Click the first patient card to load their full profile
      await page.locator('div.cards button.card.left').first().click();

      // Wait for the demographics section to appear
      await expect(
        page.locator('section.subpanel h4', { hasText: 'Clinical Fields' })
      ).toBeVisible({ timeout: 10_000 });

      // Admin does NOT have reveal_sensitive — allergies must show the masking sentinel
      const allergyField = page.locator('textarea[placeholder="Allergies"]');
      await expect(allergyField).toBeVisible({ timeout: 5_000 });
      await expect(allergyField).toHaveValue('[REDACTED - privileged reveal required]');
    }

    // ── 7. No error banners ──────────────────────────────────────────
    await expect(page.locator('p.error')).not.toBeVisible();
  });
});
