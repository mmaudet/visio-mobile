import { test, expect } from '@playwright/test';

// TODO: rewrite for the new DeskSidebar-based navigation.
// The legacy home/settings UI (HomeView + SettingsView) targeted by this suite
// was removed in the desktop UI iteration. The new SettingsScreen is reached
// via the persistent left-side DeskSidebar (data-testid TBD on the settings
// nav item), not via a `home-settings-button` on the home page, and the
// language picker is no longer a `<select>`. Re-enable + rewrite once the
// new selectors stabilise.
test.describe.skip('Settings', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.getByTestId('home-settings-button').click();
  });

  test('can change display name', async ({ page }) => {
    const input = page.getByTestId('settings-display-name-input');
    await input.clear();
    await input.fill('New Name');
    await expect(input).toHaveValue('New Name');
  });

  test('language options are visible', async ({ page }) => {
    await expect(page.getByTestId('settings-language-select')).toBeVisible();
    // Verify language options exist within the select
    const select = page.getByTestId('settings-language-select');
    await expect(select.locator('option[data-testid="settings-language-en"]')).toBeAttached();
    await expect(select.locator('option[data-testid="settings-language-fr"]')).toBeAttached();
  });

  test('can select language', async ({ page }) => {
    const select = page.getByTestId('settings-language-select');
    await select.selectOption('fr');
    await expect(select).toHaveValue('fr');
  });
});
