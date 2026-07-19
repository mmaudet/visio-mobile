import { test, expect, type Page } from '@playwright/test';
import { mockTauriCall } from './tauri-mock';

/**
 * Regression test for the desktop OIDC flow from the join screen.
 *
 * When a room requires authentication, validate_room returns "auth_required"
 * and the join button is replaced by a sign-in button. That button must start
 * the system-browser OIDC flow (launch_oidc_browser), and the room must be
 * re-validated once the visio://auth-callback deep link brings the exchange
 * code back to the app.
 */
test.describe('Join flow requiring OIDC authentication', () => {
  test.beforeEach(async ({ page }) => {
    await mockTauriCall(page, { roomRequiresAuth: true });
    await page.goto('/');
    await page
      .getByTestId('home-room-url-input')
      .fill('https://meet.example.com/abc-defg-hij');
    // When the room requires auth, the join button is replaced by the
    // sign-in button and the status message is shown.
    await expect(page.getByTestId('home-join-button')).toHaveCount(0);
    await expect(page.getByTestId('home-room-status').first()).toBeVisible();
  });

  // The sign-in button replaces the join button right after the display
  // name field when the room requires authentication.
  const signInButton = (page: Page) =>
    page
      .getByTestId('home-display-name-input')
      .locator('xpath=following::button[1]');

  test('sign-in button launches the OIDC browser flow for the room instance', async ({
    page,
  }) => {
    await signInButton(page).click();

    await expect
      .poll(async () =>
        page.evaluate(() =>
          (window as any).__invokeLog.some(
            (entry: any) =>
              entry.cmd === 'launch_oidc_browser' &&
              entry.args?.meetInstance === 'meet.example.com',
          ),
        ),
      )
      .toBe(true);
  });

  test('room is re-validated after the OIDC callback completes', async ({
    page,
  }) => {
    await signInButton(page).click();

    // Simulate the system browser redirecting back to the app with a code.
    await page.evaluate(() =>
      (window as any).__emitTauriEvent('deep-link://new-url', [
        'visio://auth-callback?code=fake-code',
      ]),
    );

    // After the code exchange, the room re-validates and joining is possible.
    await expect(page.getByTestId('home-join-button')).toBeVisible();
    await expect(page.getByTestId('home-join-button')).toBeEnabled();
  });
});
