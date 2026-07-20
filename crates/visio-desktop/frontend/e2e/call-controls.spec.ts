import { test, expect, type Page } from '@playwright/test';
import { getInvokeLog, joinMockRoom } from './tauri-mock';

/** Last recorded args of a command, or undefined if it was never called. */
async function lastInvokeArgs(page: Page, cmd: string) {
  const log = (await getInvokeLog(page)).filter((e) => e.cmd === cmd);
  return log.at(-1)?.args;
}

test.describe('Call Controls', () => {
  test.beforeEach(async ({ page }) => {
    await joinMockRoom(page);
  });

  test('all control buttons are visible', async ({ page }) => {
    await expect(page.getByTestId('call-mic-button')).toBeVisible();
    await expect(page.getByTestId('call-camera-button')).toBeVisible();
    await expect(page.getByTestId('call-screen-share-button')).toBeVisible();
    await expect(page.getByTestId('call-chat-button')).toBeVisible();
    await expect(page.getByTestId('call-hangup-button')).toBeVisible();
  });

  test('mic and camera chevrons are visible', async ({ page }) => {
    await expect(page.getByTestId('call-mic-chevron')).toBeVisible();
    await expect(page.getByTestId('call-camera-chevron')).toBeVisible();
  });

  test('can toggle microphone', async ({ page }) => {
    // Mic is enabled when the call starts (joinMockRoom joins with mic on).
    const micBtn = page.getByTestId('call-mic-button');
    await micBtn.click(); // mute
    await expect
      .poll(async () => (await lastInvokeArgs(page, 'toggle_mic'))?.enabled)
      .toBe(false);
    await micBtn.click(); // unmute
    await expect
      .poll(async () => (await lastInvokeArgs(page, 'toggle_mic'))?.enabled)
      .toBe(true);
  });

  test('can toggle camera', async ({ page }) => {
    const camBtn = page.getByTestId('call-camera-button');
    await camBtn.click(); // off
    await expect
      .poll(async () => (await lastInvokeArgs(page, 'toggle_camera'))?.enabled)
      .toBe(false);
    await camBtn.click(); // on
    await expect
      .poll(async () => (await lastInvokeArgs(page, 'toggle_camera'))?.enabled)
      .toBe(true);
  });

  test('can toggle hand raise from the control bar', async ({ page }) => {
    // The redesign has no overflow menu: hand raise is a first-class button
    // directly in the call bar.
    const handBtn = page.getByTestId('call-hand-raise-button');
    await expect(handBtn).toBeVisible();

    await handBtn.click(); // raise hand
    await expect
      .poll(async () =>
        (await getInvokeLog(page)).some((e) => e.cmd === 'raise_hand'),
      )
      .toBe(true);

    await handBtn.click(); // lower hand
    await expect
      .poll(async () =>
        (await getInvokeLog(page)).some((e) => e.cmd === 'lower_hand'),
      )
      .toBe(true);
  });

  test('participant grid is visible', async ({ page }) => {
    await expect(page.getByTestId('call-participant-grid')).toBeVisible();
  });
});
