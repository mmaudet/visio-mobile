import { test, expect, type Locator, type Page } from '@playwright/test';
import { getInvokeLog, joinMockRoom } from './tauri-mock';

/**
 * The redesign's PickerRow has no radio input and no default-star: selection
 * is shown by a check icon whose color is var(--accent) when the row is
 * active and `transparent` otherwise. Read the icon's computed color.
 */
async function checkIconColor(row: Locator): Promise<string> {
  return row
    .locator('svg')
    .first()
    .evaluate((el) => getComputedStyle(el).color);
}

const TRANSPARENT = 'rgba(0, 0, 0, 0)';

async function expectRowSelected(row: Locator) {
  await expect.poll(async () => checkIconColor(row)).not.toBe(TRANSPARENT);
}

async function expectRowNotSelected(row: Locator) {
  await expect.poll(async () => checkIconColor(row)).toBe(TRANSPARENT);
}

async function expectInvoked(
  page: Page,
  cmd: string,
  args: Record<string, unknown>,
) {
  await expect
    .poll(async () =>
      (await getInvokeLog(page)).some(
        (e) =>
          e.cmd === cmd &&
          Object.entries(args).every(([k, v]) => e.args?.[k] === v),
      ),
    )
    .toBe(true);
}

test.describe('Device Selection', () => {
  test.beforeEach(async ({ page }) => {
    await joinMockRoom(page);
  });

  test('audio device picker shows input and output devices', async ({
    page,
  }) => {
    // Click the mic chevron to open device picker
    const chevron = page.getByTestId('call-mic-chevron');
    await chevron.click();

    // Should show device picker with audio devices
    const picker = page.getByTestId('device-picker-audio');
    await expect(picker).toBeVisible();

    // Should show microphone options
    await expect(picker.getByText('Built-in Microphone')).toBeVisible();
    await expect(picker.getByText('External USB Mic')).toBeVisible();

    // Should show speaker options
    await expect(picker.getByText('Built-in Speakers')).toBeVisible();
    await expect(picker.getByText('Headphones')).toBeVisible();
  });

  test('can select a different microphone', async ({ page }) => {
    await page.getByTestId('call-mic-chevron').click();

    const picker = page.getByTestId('device-picker-audio');
    await expect(picker).toBeVisible();

    // Click on the External USB Mic option — the redesign closes the picker
    // after a pick.
    await picker.getByText('External USB Mic').click();
    await expect(picker).not.toBeVisible();
    await expectInvoked(page, 'select_audio_input', {
      deviceName: 'External USB Mic',
    });

    // Reopen: the picked row carries the lit check icon, the previous one
    // does not.
    await page.getByTestId('call-mic-chevron').click();
    await expectRowSelected(page.getByTestId('device-option-input-1'));
    await expectRowNotSelected(page.getByTestId('device-option-input-0'));
  });

  test('can select a different speaker', async ({ page }) => {
    await page.getByTestId('call-mic-chevron').click();

    const picker = page.getByTestId('device-picker-audio');
    await expect(picker).toBeVisible();

    await picker.getByText('Headphones').click();
    await expect(picker).not.toBeVisible();
    await expectInvoked(page, 'select_audio_output', {
      deviceName: 'Headphones',
    });

    await page.getByTestId('call-mic-chevron').click();
    await expectRowSelected(page.getByTestId('device-option-output-1'));
    await expectRowNotSelected(page.getByTestId('device-option-output-0'));
  });

  test('camera device picker shows cameras', async ({ page }) => {
    const chevron = page.getByTestId('call-camera-chevron');
    await chevron.click();

    const picker = page.getByTestId('device-picker-video');
    await expect(picker).toBeVisible();

    await expect(picker.getByText('FaceTime HD Camera')).toBeVisible();
    await expect(picker.getByText('External Webcam')).toBeVisible();
  });

  test('can select a different camera', async ({ page }) => {
    await page.getByTestId('call-camera-chevron').click();

    const picker = page.getByTestId('device-picker-video');
    await expect(picker).toBeVisible();

    await picker.getByText('External Webcam').click();
    await expect(picker).not.toBeVisible();
    await expectInvoked(page, 'select_video_input', { uniqueId: 'cam-2' });

    await page.getByTestId('call-camera-chevron').click();
    await expectRowSelected(page.getByTestId('device-option-camera-1'));
    await expectRowNotSelected(page.getByTestId('device-option-camera-0'));
  });

  test('clicking outside device picker closes it', async ({ page }) => {
    await page.getByTestId('call-mic-chevron').click();
    await expect(page.getByTestId('device-picker-audio')).toBeVisible();

    // Click somewhere outside (the participant grid area)
    await page.getByTestId('call-participant-grid').click({ force: true });

    // Picker should close
    await expect(page.getByTestId('device-picker-audio')).not.toBeVisible();
  });

  test('default devices are pre-selected when the picker opens', async ({
    page,
  }) => {
    // The redesign dropped the ★ default marker; what remains is that the
    // default device is the active selection on a fresh join (the mock marks
    // "Built-in Microphone"/"Built-in Speakers" as is_default).
    await page.getByTestId('call-mic-chevron').click();

    await expectRowSelected(page.getByTestId('device-option-input-0'));
    await expectRowSelected(page.getByTestId('device-option-output-0'));
    await expectRowNotSelected(page.getByTestId('device-option-input-1'));
    await expectRowNotSelected(page.getByTestId('device-option-output-1'));
  });

  test('opening mic picker closes camera picker', async ({ page }) => {
    // Open camera picker first
    await page.getByTestId('call-camera-chevron').click();
    await expect(page.getByTestId('device-picker-video')).toBeVisible();

    // Now open mic picker
    await page.getByTestId('call-mic-chevron').click();
    await expect(page.getByTestId('device-picker-audio')).toBeVisible();
    await expect(page.getByTestId('device-picker-video')).not.toBeVisible();
  });
});
