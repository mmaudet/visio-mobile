import { test, expect } from '@playwright/test';
import { getInvokeLog, joinMockRoom } from './tauri-mock';

test.describe('Chat', () => {
  test.beforeEach(async ({ page }) => {
    await joinMockRoom(page);
  });

  test('chat button opens chat sidebar', async ({ page }) => {
    await page.getByTestId('call-chat-button').click();
    await expect(page.getByTestId('call-chat-sidebar')).toBeVisible();
  });

  test('chat shows empty state when no messages', async ({ page }) => {
    await page.getByTestId('call-chat-button').click();
    await expect(page.getByTestId('chat-empty')).toBeVisible();
  });

  test('can type and send a chat message', async ({ page }) => {
    await page.getByTestId('call-chat-button').click();

    const input = page.getByTestId('chat-message-input');
    await input.fill('Hello from Playwright');

    await page.getByTestId('chat-send-button').click();

    // Input should be cleared after send
    await expect(input).toHaveValue('');
    // The text is handed to the backend and the message list (refreshed by
    // the 1s poll) ends up showing our own bubble.
    await expect
      .poll(async () =>
        (await getInvokeLog(page)).some(
          (e) =>
            e.cmd === 'send_chat' && e.args?.text === 'Hello from Playwright',
        ),
      )
      .toBe(true);
    await expect(page.getByText('Hello from Playwright')).toBeVisible();
    await expect(page.getByTestId('chat-empty')).not.toBeVisible();
  });

  test('clicking send with an empty or blank input sends nothing', async ({
    page,
  }) => {
    // The redesign's send button is never disabled: the guard lives in the
    // submit handler (blank drafts are dropped). Verify the guard holds.
    await page.getByTestId('call-chat-button').click();

    await page.getByTestId('chat-send-button').click();
    // Extra round-trip time to let any (erroneous) send fire.
    await page.waitForTimeout(300);
    expect(
      (await getInvokeLog(page)).some((e) => e.cmd === 'send_chat'),
    ).toBe(false);

    // Whitespace-only drafts are trimmed away and must not send either.
    const input = page.getByTestId('chat-message-input');
    await input.fill('   ');
    await page.getByTestId('chat-send-button').click();
    await page.waitForTimeout(300);
    expect(
      (await getInvokeLog(page)).some((e) => e.cmd === 'send_chat'),
    ).toBe(false);
    await expect(page.getByTestId('chat-empty')).toBeVisible();
  });

  test('can close chat sidebar', async ({ page }) => {
    await page.getByTestId('call-chat-button').click();
    await expect(page.getByTestId('call-chat-sidebar')).toBeVisible();

    await page.getByTestId('chat-close-button').click();
    await expect(page.getByTestId('call-chat-sidebar')).not.toBeVisible();
  });

  test('can send message with Enter key', async ({ page }) => {
    await page.getByTestId('call-chat-button').click();

    const input = page.getByTestId('chat-message-input');
    await input.fill('Hello via Enter');
    await input.press('Enter');

    // Input should be cleared after send
    await expect(input).toHaveValue('');
    await expect
      .poll(async () =>
        (await getInvokeLog(page)).some(
          (e) => e.cmd === 'send_chat' && e.args?.text === 'Hello via Enter',
        ),
      )
      .toBe(true);
  });

  test('chat message list is visible', async ({ page }) => {
    await page.getByTestId('call-chat-button').click();
    await expect(page.getByTestId('chat-message-list')).toBeVisible();
  });
});

test.describe('Chat with pre-existing messages', () => {
  test('shows existing messages instead of empty state', async ({ page }) => {
    await joinMockRoom(page, {
      messages: [
        {
          id: 'msg-1',
          text: 'Hello!',
          sender_sid: 'PA_remote1',
          sender_name: 'E2E Bot',
          timestamp_ms: Date.now() - 60000,
        },
        {
          id: 'msg-2',
          text: 'Hi there!',
          sender_sid: 'PA_local',
          sender_name: 'Test User',
          timestamp_ms: Date.now() - 30000,
        },
      ],
    });

    await page.getByTestId('call-chat-button').click();

    // Should NOT show empty state
    await expect(page.getByTestId('chat-empty')).not.toBeVisible();

    // Should show messages
    await expect(page.getByTestId('chat-message-list')).toBeVisible();
    await expect(page.getByText('Hello!')).toBeVisible();
    await expect(page.getByText('Hi there!')).toBeVisible();
  });

  test('messages have bubble test ids', async ({ page }) => {
    await joinMockRoom(page, {
      messages: [
        {
          id: 'msg-1',
          text: 'First message',
          sender_sid: 'PA_remote1',
          sender_name: 'E2E Bot',
          timestamp_ms: Date.now() - 60000,
        },
        {
          id: 'msg-2',
          text: 'Second message',
          sender_sid: 'PA_local',
          sender_name: 'Test User',
          timestamp_ms: Date.now() - 30000,
        },
      ],
    });

    await page.getByTestId('call-chat-button').click();

    await expect(page.getByTestId('chat-bubble-0')).toBeVisible();
    await expect(page.getByTestId('chat-bubble-1')).toBeVisible();
  });
});
