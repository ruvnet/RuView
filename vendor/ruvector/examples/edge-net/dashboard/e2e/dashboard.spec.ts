import { test, expect } from '@playwright/test';

test.describe('EdgeNet Dashboard', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    // Wait for loading to complete
    await page.waitForSelector('text=Network Overview', { timeout: 15000 });
  });

  test('loads successfully with Network Overview', async ({ page }) => {
    await expect(page.locator('text=Network Overview')).toBeVisible();
    await expect(page.locator('text=Credits Earned').first()).toBeVisible();
  });

  test('displays credits summary cards', async ({ page }) => {
    await expect(page.locator('text=Credits Earned').first()).toBeVisible();
    await expect(page.locator('text=Available').first()).toBeVisible();
    await expect(page.locator('text=Peers Online').first()).toBeVisible();
    await expect(page.locator('text=Status').first()).toBeVisible();
  });

  test('navigates to AI Agents page', async ({ page }) => {
    await page.click('text=AI Agents');
    await expect(page.locator('h1:has-text("AI Agents")')).toBeVisible({ timeout: 10000 });
    // Shows real MCP agents only (no demos)
    // If no MCP tools connected, shows empty state with "No agents" message
  });

  test('navigates to Workers page and shows local worker', async ({ page }) => {
    await page.click('text=Workers');
    await expect(page.locator('h1:has-text("Compute Workers")')).toBeVisible({ timeout: 10000 });
    // Should show local worker
    await expect(page.locator('text=Local Node')).toBeVisible({ timeout: 10000 });
    await expect(page.locator('text=Active Workers')).toBeVisible();
  });

  test('navigates to Plugins page and shows WASM plugins', async ({ page }) => {
    await page.click('text=Plugins');
    await expect(page.locator('h1:has-text("Plugin Manager")')).toBeVisible({ timeout: 10000 });
    // Should show real WASM plugins
    await expect(page.locator('text=@ruvector/edge-net').first()).toBeVisible({ timeout: 10000 });
    await expect(page.locator('text=RuVector Team').first()).toBeVisible();
  });

  test('navigates to Network & Communities page', async ({ page }) => {
    await page.click('text=Network');
    await expect(page.locator('h1:has-text("Network & Communities")')).toBeVisible({ timeout: 10000 });
    await expect(page.locator('text=Credits Earned').first()).toBeVisible();
  });

  test('navigates to Activity page and shows activity log', async ({ page }) => {
    await page.click('text=Activity');
    await expect(page.locator('h1:has-text("Activity Log")')).toBeVisible({ timeout: 10000 });
    await expect(page.locator('text=Total Events')).toBeVisible();
  });

  test('navigates to Settings page and shows settings', async ({ page }) => {
    await page.click('text=Settings');
    await expect(page.locator('h1:has-text("Settings")')).toBeVisible({ timeout: 10000 });
    await expect(page.locator('text=Contribution Settings')).toBeVisible({ timeout: 10000 });
    await expect(page.locator('text=Enable Contribution')).toBeVisible();
  });

  test('navigates to Credits page', async ({ page }) => {
    await page.click('text=Credits');
    await expect(page.locator('h1:has-text("Credit Economy")')).toBeVisible({ timeout: 10000 });
  });

  test('consent widget is visible', async ({ page }) => {
    // Consent widget should be visible at bottom
    const consentWidget = page.locator('.fixed.bottom-4');
    await expect(consentWidget).toBeVisible({ timeout: 10000 });
  });

  test('Identity & Networks modal shows correctly', async ({ page }) => {
    await page.click('text=Identity');
    await expect(page.locator('h1:has-text("Identity & Networks")')).toBeVisible({ timeout: 10000 });
    // Click on a network to join
    const joinButton = page.locator('text=Join Network').first();
    if (await joinButton.isVisible()) {
      await joinButton.click();
      // Modal should be visible and centered
      const modal = page.locator('.fixed.left-1\\/2.top-1\\/2');
      await expect(modal).toBeVisible({ timeout: 5000 });
    }
  });

  test('Genesis page loads', async ({ page }) => {
    await page.click('text=Genesis');
    await expect(page.locator('text=Genesis')).toBeVisible({ timeout: 15000 });
  });
});
