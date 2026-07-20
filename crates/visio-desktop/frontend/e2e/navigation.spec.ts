import { test, expect } from '@playwright/test';
import { mockTauriCall } from './tauri-mock';

test.describe('Navigation', () => {
  test.beforeEach(async ({ page }) => {
    await mockTauriCall(page);
  });

  test('app loads home screen by default', async ({ page }) => {
    await page.goto('/');
    await expect(page.getByTestId('home-room-url-input')).toBeVisible();
    await expect(page.getByTestId('home-join-button')).toBeVisible();
  });

  test('settings is a full page, not a modal overlay', async ({ page }) => {
    await page.goto('/');
    await page.getByTestId('sidebar-settings-link').click();
    // The redesign replaces the home screen entirely instead of layering a
    // modal on top of it: the settings page shows and Home is unmounted.
    await expect(
      page.getByTestId('settings-display-name-trigger'),
    ).toBeVisible();
    await expect(page.getByTestId('home-room-url-input')).not.toBeVisible();
  });
});
