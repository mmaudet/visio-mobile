import { test, expect, type Page } from '@playwright/test';
import { mockTauriCall, type MockCallState } from './tauri-mock';

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
  // The sign-in button replaces the join button when the room requires
  // authentication.
  const signInButton = (page: Page) => page.getByTestId('home-signin-button');

  const reachAuthRequiredRoom = async (
    page: Page,
    overrides: MockCallState = {},
  ) => {
    await mockTauriCall(page, { roomRequiresAuth: true, ...overrides });
    await page.goto('/');
    await page
      .getByTestId('home-room-url-input')
      .fill('https://meet.example.com/abc-defg-hij');
    // When the room requires auth, the join button is replaced by the
    // sign-in button and the status padlock is shown (redesign display).
    await expect(page.getByTestId('home-join-button')).toHaveCount(0);
    await expect(signInButton(page)).toBeVisible();
    await expect(page.getByTestId('home-room-status').first()).toBeVisible();
  };

  test.beforeEach(async ({ page }) => {
    await reachAuthRequiredRoom(page);
  });

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
        'visio://auth-callback?code=fake-code&state=fake-state',
      ]),
    );

    // After the code exchange, the room re-validates and joining is possible.
    await expect(page.getByTestId('home-join-button')).toBeVisible();
    await expect(page.getByTestId('home-join-button')).toBeEnabled();
  });

  test('returns to sign-in when the OIDC exchange fails', async ({ page }) => {
    await reachAuthRequiredRoom(page, { oidcExchangeFails: true });
    await signInButton(page).click();

    // Simulate the system browser redirecting back with a code whose
    // exchange fails (e.g. expired code).
    await page.evaluate(() =>
      (window as any).__emitTauriEvent('deep-link://new-url', [
        'visio://auth-callback?code=fake-code&state=fake-state',
      ]),
    );

    // Wait until the failing exchange has actually been attempted (i.e. the
    // UI has entered the "authenticating" state).
    await expect
      .poll(async () =>
        page.evaluate(() =>
          (window as any).__invokeLog.some(
            (entry: any) => entry.cmd === 'exchange_pkce_code',
          ),
        ),
      )
      .toBe(true);

    // The UI must recover to the sign-in state instead of staying stuck on
    // "Authenticating…" forever.
    await expect(signInButton(page)).toBeVisible();
  });

  test('returns to sign-in when launching the OIDC browser fails', async ({
    page,
  }) => {
    await reachAuthRequiredRoom(page, { oidcLaunchFails: true });
    await signInButton(page).click();

    // Wait until the failing browser launch has actually been attempted (i.e.
    // the UI has entered the "authenticating" state).
    await expect
      .poll(async () =>
        page.evaluate(() =>
          (window as any).__invokeLog.some(
            (entry: any) => entry.cmd === 'launch_oidc_browser',
          ),
        ),
      )
      .toBe(true);

    // The browser cannot be opened: the UI must recover to the sign-in state
    // instead of staying stuck on "Authenticating…" forever.
    await expect(signInButton(page)).toBeVisible();
  });

  test('shows an error and recovers when the OIDC callback never arrives', async ({
    page,
  }) => {
    // Shorten the app-side callback timeout for the test (the production
    // value is 120 s — the app honors this test hook).
    await page.evaluate(() => {
      (window as any).__OIDC_TIMEOUT_MS = 500;
    });
    await signInButton(page).click();

    // No deep link ever arrives (e.g. an instance without PKCE support):
    // the app must surface an error and offer sign-in again instead of
    // waiting silently forever.
    await expect(page.getByTestId('home-auth-error')).toBeVisible({
      timeout: 5000,
    });
    await expect(signInButton(page)).toBeVisible();
  });
});
