import { test, expect } from '@playwright/test';
import { mockTauriCall } from './tauri-mock';

test.describe('Home Screen', () => {
  test.beforeEach(async ({ page }) => {
    await mockTauriCall(page);
    await page.goto('/');
  });

  test('displays room URL input and join button', async ({ page }) => {
    await expect(page.getByTestId('home-room-url-input')).toBeVisible();
    await expect(page.getByTestId('home-join-button')).toBeVisible();
  });

  test('displays settings link in the sidebar', async ({ page }) => {
    // The redesign has no settings button on Home: settings lives in the
    // persistent DeskSidebar.
    await expect(page.getByTestId('sidebar-settings-link')).toBeVisible();
  });

  test('clicking join with an empty URL stays on the home screen', async ({
    page,
  }) => {
    // The redesign keeps the join button enabled while idle; the guard is in
    // the submit handler (empty code is a no-op), so no navigation happens.
    await page.getByTestId('home-join-button').click();
    await expect(page.getByTestId('home-room-url-input')).toBeVisible();
    await expect(page.getByTestId('prejoin-join-button')).toHaveCount(0);
  });

  test('join button is disabled when the room is not found', async ({
    page,
  }) => {
    // A value that is neither a URL nor a room slug fails validation
    // (status "not_found") and the redesign disables the join button.
    await page.getByTestId('home-room-url-input').fill('not-a-valid-room');
    await expect(page.getByTestId('home-join-button')).toBeDisabled();
    await expect(page.getByTestId('home-room-status').first()).toBeVisible();
  });

  test('can enter room URL', async ({ page }) => {
    const input = page.getByTestId('home-room-url-input');
    await input.fill('https://meet.example.com/abc-defg-hij');
    await expect(input).toHaveValue('https://meet.example.com/abc-defg-hij');
  });

  test('settings page opens and closes via the sidebar', async ({ page }) => {
    // Settings is a full page in the redesign: navigate to it through the
    // sidebar, then back to Home through the sidebar Home nav item.
    await page.getByTestId('sidebar-settings-link').click();
    await expect(
      page.getByTestId('settings-display-name-trigger'),
    ).toBeVisible();

    await page.getByRole('button', { name: 'Home', exact: true }).click();
    await expect(page.getByTestId('home-room-url-input')).toBeVisible();
    await expect(
      page.getByTestId('settings-display-name-trigger'),
    ).not.toBeVisible();
  });
});
