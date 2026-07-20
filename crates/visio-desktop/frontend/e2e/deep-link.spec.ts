import { test, expect } from '@playwright/test';
import { mockTauriCall } from './tauri-mock';

/**
 * Regression test for the visio:// deep-link prefill on the home screen.
 *
 * When the OS opens a visio://{host}/{slug} URL, the app must resolve it
 * against the known Meet instances, prefill the home join field with the
 * https room URL, and let the debounced validation run so the room can be
 * joined without retyping anything.
 */
test.describe('Deep-link room prefill', () => {
  test('visio:// link for a known instance prefills the join input', async ({
    page,
  }) => {
    await mockTauriCall(page, { meetInstances: ['meet.example.com'] });
    await page.goto('/');

    const urlInput = page.getByTestId('home-room-url-input');
    // Ensure the app has mounted and registered its deep-link listener
    // before emitting — an event emitted earlier would be dropped.
    await urlInput.waitFor({ state: 'attached', timeout: 5000 });

    // Simulate the OS delivering a visio:// room link to the app.
    await page.evaluate(() =>
      (window as any).__emitTauriEvent('deep-link://new-url', [
        'visio://meet.example.com/abc-defg-hij',
      ]),
    );

    // The join field is prefilled with the resolved https room URL…
    await expect(urlInput).toHaveValue(
      'https://meet.example.com/abc-defg-hij',
    );

    // …and the debounced validation kicks in: once the room is validated,
    // the join button (disabled while idle/checking/not_found) enables.
    await expect(page.getByTestId('home-join-button')).toBeEnabled();
  });

  test('visio:// link for an unknown instance shows an error instead', async ({
    page,
  }) => {
    await mockTauriCall(page, { meetInstances: ['meet.example.com'] });
    await page.goto('/');

    const urlInput = page.getByTestId('home-room-url-input');
    await urlInput.waitFor({ state: 'attached', timeout: 5000 });

    await page.evaluate(() =>
      (window as any).__emitTauriEvent('deep-link://new-url', [
        'visio://unknown.example.com/abc-defg-hij',
      ]),
    );

    // The field stays empty — joining an unknown instance is rejected.
    await expect(page.getByTestId('home-error-banner')).toBeVisible();
    await expect(urlInput).toHaveValue('');
  });
});
