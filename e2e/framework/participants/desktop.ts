import { chromium, type Browser, type Page } from "playwright-core";
import { join } from "node:path";
import { pollUntil } from "../assertions/poll.js";
import { captureDesktop } from "../evidence/screenshot.js";
import type { DesktopParticipant } from "./types.js";

const DEFAULT_TIMEOUT = 5_000;

interface DesktopOptions {
  identity: string;
  livekitUrl: string;
  token: string;
  screenshotDir: string;
}

export class DesktopParticipantImpl implements DesktopParticipant {
  readonly identity: string;

  private readonly _livekitUrl: string;
  private readonly _token: string;
  private readonly _screenshotDir: string;
  private _browser: Browser | null = null;
  private _page: Page | null = null;

  constructor(opts: DesktopOptions) {
    this.identity = opts.identity;
    this._livekitUrl = opts.livekitUrl;
    this._token = opts.token;
    this._screenshotDir = opts.screenshotDir;
  }

  async connect(): Promise<void> {
    // NOTE: E2E desktop testing uses the React/Vite web app (visio-desktop) served
    // at localhost:5173 via Playwright, NOT the native Tauri application binary.
    // This allows fast, headful browser-based assertions without requiring a packaged
    // Tauri build.  The Tauri app itself is tested separately via its own test harness.
    this._browser = await chromium.launch({ headless: false });
    const context = await this._browser.newContext();
    this._page = await context.newPage();

    const url = new URL("http://localhost:5173");
    url.searchParams.set("livekit_url", this._livekitUrl);
    url.searchParams.set("token", this._token);

    await this._page.goto(url.toString(), { waitUntil: "domcontentloaded" });
  }

  async assertTestId(testId: string, options?: { timeout?: number }): Promise<void> {
    const timeout = options?.timeout ?? DEFAULT_TIMEOUT;
    await pollUntil(
      async () => {
        const page = this._requirePage();
        return page.locator(`[data-testid="${testId}"]`).isVisible();
      },
      {
        timeout,
        description: `assertTestId("${testId}")`,
      },
    );
  }

  async assertNotTestId(testId: string, options?: { timeout?: number }): Promise<void> {
    const timeout = options?.timeout ?? DEFAULT_TIMEOUT;
    await pollUntil(
      async () => {
        const page = this._requirePage();
        const visible = await page.locator(`[data-testid="${testId}"]`).isVisible();
        return !visible;
      },
      {
        timeout,
        description: `assertNotTestId("${testId}")`,
      },
    );
  }

  async click(testId: string): Promise<void> {
    const page = this._requirePage();
    await page.locator(`[data-testid="${testId}"]`).click();
  }

  async screenshot(name: string): Promise<string> {
    const page = this._requirePage();
    const outputPath = join(this._screenshotDir, `${name}.png`);
    return captureDesktop(page, outputPath);
  }

  async disconnect(): Promise<void> {
    if (this._browser) {
      await this._browser.close();
      this._browser = null;
      this._page = null;
    }
  }

  // ── Private helpers ──

  private _requirePage(): Page {
    if (!this._page) {
      throw new Error("Desktop participant not connected — call connect() first");
    }
    return this._page;
  }
}
