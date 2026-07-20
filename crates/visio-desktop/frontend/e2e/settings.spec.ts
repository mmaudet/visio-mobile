import { test, expect } from '@playwright/test';
import { getInvokeLog, mockTauriCall } from './tauri-mock';

test.describe('Settings', () => {
  test.beforeEach(async ({ page }) => {
    await mockTauriCall(page);
    await page.goto('/');
    // Settings is a full page reached via the persistent DeskSidebar.
    await page.getByTestId('sidebar-settings-link').click();
    await expect(
      page.getByTestId('settings-display-name-trigger'),
    ).toBeVisible();
  });

  test('can change display name', async ({ page }) => {
    // The display name row opens an InlineEditor popover.
    await page.getByTestId('settings-display-name-trigger').click();
    const input = page.getByTestId('settings-display-name-input');
    await expect(input).toBeVisible();

    await input.fill('New Name');
    await input.press('Enter');

    // The editor closes, the new name is persisted via set_display_name and
    // the profile row reflects it.
    await expect(input).not.toBeVisible();
    await expect(
      page.getByTestId('settings-display-name-trigger'),
    ).toContainText('New Name');
    await expect
      .poll(async () =>
        (await getInvokeLog(page)).some(
          (e) => e.cmd === 'set_display_name' && e.args?.name === 'New Name',
        ),
      )
      .toBe(true);
  });

  test('language picker lists all supported languages', async ({ page }) => {
    // The language row opens a PopoverMenu (not a <select>).
    await page.getByTestId('settings-language-select').click();
    for (const code of ['en', 'fr', 'de', 'es', 'it', 'nl']) {
      await expect(page.getByTestId(`settings-language-${code}`)).toBeVisible();
    }
  });

  test('can change the language', async ({ page }) => {
    await page.getByTestId('settings-language-select').click();
    await page.getByTestId('settings-language-fr').click();

    // The popover closes, the choice is persisted via set_language and the
    // row's trailing label switches to the picked language.
    await expect(page.getByTestId('settings-language-en')).not.toBeVisible();
    await expect
      .poll(async () =>
        (await getInvokeLog(page)).some(
          (e) => e.cmd === 'set_language' && e.args?.lang === 'fr',
        ),
      )
      .toBe(true);
    await expect(
      page.getByTestId('settings-language-select'),
    ).toContainText('Français');
  });

  test('can switch the theme', async ({ page }) => {
    const dark = page.getByRole('button', { name: 'Dark', exact: true });
    await expect(dark).toBeVisible();
    await dark.click();

    // The segmented control marks the picked theme and set_theme is called.
    await expect(dark).toHaveClass(/\bon\b/);
    await expect
      .poll(async () =>
        (await getInvokeLog(page)).some(
          (e) => e.cmd === 'set_theme' && e.args?.theme === 'dark',
        ),
      )
      .toBe(true);
  });
});
