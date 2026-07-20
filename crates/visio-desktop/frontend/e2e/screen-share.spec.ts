import { test, expect } from '@playwright/test';
import { joinMockRoom } from './tauri-mock';

test.describe('Screen Share', () => {
  test.beforeEach(async ({ page }) => {
    await joinMockRoom(page);
  });

  test('screen share button opens source picker', async ({ page }) => {
    await page.getByTestId('call-screen-share-button').click();

    const picker = page.getByTestId('screen-share-source-picker');
    await expect(picker).toBeVisible();
  });

  test('source picker shows monitors and windows', async ({ page }) => {
    await page.getByTestId('call-screen-share-button').click();

    const picker = page.getByTestId('screen-share-source-picker');
    await expect(picker.getByText('Main Display')).toBeVisible();
    await expect(picker.getByText('Browser')).toBeVisible();
    await expect(picker.getByText('Terminal')).toBeVisible();
  });

  test('source items have correct test ids', async ({ page }) => {
    await page.getByTestId('call-screen-share-button').click();

    // Monitor is index 0
    await expect(page.getByTestId('screen-share-source-0')).toBeVisible();
    // Windows are index 1 and 2 (monitors.length + window index)
    await expect(page.getByTestId('screen-share-source-1')).toBeVisible();
    await expect(page.getByTestId('screen-share-source-2')).toBeVisible();
  });

  test('can select a screen source to share', async ({ page }) => {
    await page.getByTestId('call-screen-share-button').click();

    // Click on Main Display source
    await page.getByTestId('screen-share-source-0').click();

    // Source picker should close after selection
    await expect(
      page.getByTestId('screen-share-source-picker'),
    ).not.toBeVisible();
  });

  test('can close source picker by clicking overlay', async ({ page }) => {
    await page.getByTestId('call-screen-share-button').click();
    const picker = page.getByTestId('screen-share-source-picker');
    await expect(picker).toBeVisible();

    // The redesign's overlay has no .modal-overlay class: it is the picker's
    // full-screen parent div, whose onClick closes the picker (the modal
    // content stops propagation). Click a top-left corner of it.
    const overlay = picker.locator('..');
    await overlay.click({ position: { x: 5, y: 5 } });

    await expect(picker).not.toBeVisible();
  });
});
