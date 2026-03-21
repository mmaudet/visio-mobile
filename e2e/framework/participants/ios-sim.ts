import { execSync } from "node:child_process";
import { join } from "node:path";
import { captureIosSim } from "../evidence/screenshot.js";
import type { IosSimParticipant } from "./types.js";

const BUNDLE_ID = "io.visio.mobile";
const LAUNCH_SETTLE_MS = 3_000;

interface IosSimOptions {
  identity: string;
  livekitUrl: string;
  token: string;
  screenshotDir: string;
}

export class IosSimParticipantImpl implements IosSimParticipant {
  readonly identity: string;

  private readonly _livekitUrl: string;
  private readonly _token: string;
  private readonly _screenshotDir: string;

  constructor(opts: IosSimOptions) {
    this.identity = opts.identity;
    this._livekitUrl = opts.livekitUrl;
    this._token = opts.token;
    this._screenshotDir = opts.screenshotDir;
  }

  async connect(): Promise<void> {
    const deepLink =
      `visio-test://connect?livekit_url=${encodeURIComponent(this._livekitUrl)}` +
      `&token=${encodeURIComponent(this._token)}` +
      `&skip_prejoin=true`;

    execSync(`xcrun simctl openurl booted "${deepLink}"`, { timeout: 10_000 });

    // Give the app time to launch and connect
    await new Promise((resolve) => setTimeout(resolve, LAUNCH_SETTLE_MS));
  }

  async screenshot(name: string): Promise<string> {
    const outputPath = join(this._screenshotDir, `${name}.png`);
    return captureIosSim(outputPath);
  }

  async disconnect(): Promise<void> {
    try {
      execSync(`xcrun simctl terminate booted ${BUNDLE_ID}`, { timeout: 10_000 });
    } catch {
      // App may already be terminated — ignore.
    }
  }
}
