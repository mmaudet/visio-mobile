import { join } from "node:path";
import * as adb from "../utils/adb.js";
import { getLocalIp } from "../utils/livekit.js";
import { pollUntil } from "../assertions/poll.js";
import { captureAndroid } from "../evidence/screenshot.js";
import type { AndroidParticipant } from "./types.js";

const DEFAULT_TIMEOUT = 5_000;
const LAUNCH_SETTLE_MS = 3_000;

interface AndroidOptions {
  identity: string;
  livekitUrl: string;
  token: string;
  screenshotDir: string;
}

export class AndroidParticipantImpl implements AndroidParticipant {
  readonly identity: string;

  private readonly _livekitUrl: string;
  private readonly _token: string;
  private readonly _screenshotDir: string;
  private _connected = false;
  private _connecting: Promise<void> | null = null;

  constructor(opts: AndroidOptions) {
    this.identity = opts.identity;
    this._livekitUrl = opts.livekitUrl;
    this._token = opts.token;
    this._screenshotDir = opts.screenshotDir;
  }

  private async _ensureConnected(): Promise<void> {
    if (this._connected) return;
    if (this._connecting) { await this._connecting; return; }
    this._connecting = this.connect();
    await this._connecting;
    this._connecting = null;
  }

  async connect(): Promise<void> {
    // Replace localhost with local IP so Android device can reach the host
    let androidUrl = this._livekitUrl;
    if (androidUrl.includes("localhost") || androidUrl.includes("127.0.0.1")) {
      const localIp = getLocalIp();
      androidUrl = androidUrl.replace(/localhost|127\.0\.0\.1/, localIp);
    }
    const deepLink =
      `visio-test://connect?livekit_url=${encodeURIComponent(androidUrl)}` +
      `&token=${encodeURIComponent(this._token)}` +
      `&skip_prejoin=true`;

    adb.launchDeepLink(deepLink);

    // Give the app time to launch and connect
    await new Promise((resolve) => setTimeout(resolve, LAUNCH_SETTLE_MS));
    this._connected = true;
  }

  async assertTestTag(tag: string, options?: { timeout?: number }): Promise<void> {
    await this._ensureConnected();
    const timeout = options?.timeout ?? DEFAULT_TIMEOUT;
    await pollUntil(
      () => adb.hasTestTag(tag),
      {
        timeout,
        description: `assertTestTag("${tag}")`,
      },
    );
  }

  async assertNotTestTag(tag: string, options?: { timeout?: number }): Promise<void> {
    await this._ensureConnected();
    const timeout = options?.timeout ?? DEFAULT_TIMEOUT;
    await pollUntil(
      () => !adb.hasTestTag(tag),
      {
        timeout,
        description: `assertNotTestTag("${tag}")`,
      },
    );
  }

  async tap(tag: string): Promise<void> {
    await this._ensureConnected();
    adb.tapTestTag(tag);
  }

  async longPress(tag: string): Promise<void> {
    await this._ensureConnected();
    adb.longPressTestTag(tag);
  }

  async dumpUiTree(): Promise<string> {
    await this._ensureConnected();
    return adb.dumpUiTree();
  }

  async screenshot(name: string): Promise<string> {
    await this._ensureConnected();
    const outputPath = join(this._screenshotDir, `${name}.png`);
    return captureAndroid(outputPath);
  }

  async disconnect(): Promise<void> {
    adb.forceStop();
  }
}
