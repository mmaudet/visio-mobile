import { execSync } from "node:child_process";
import { resolve } from "node:path";
import * as livekit from "./utils/livekit.js";
import * as adb from "./utils/adb.js";

export interface PlatformAvailability {
  android: boolean;
  desktop: boolean;
  ios: boolean;
}

export interface OrchestratorConfig {
  projectRoot: string;
  livekitUrl: string;
  room: string;
  apiKey: string;
  apiSecret: string;
}

/**
 * Return a default config suitable for local development.
 */
export function getConfig(): OrchestratorConfig {
  return {
    projectRoot: resolve(import.meta.dirname, "../.."),
    livekitUrl: "ws://localhost:7880",
    room: "e2e-room",
    apiKey: "devkey",
    apiSecret: "secret",
  };
}

/**
 * Detect whether an iOS simulator is currently booted.
 */
function isIosSimBooted(): boolean {
  try {
    const out = execSync("xcrun simctl list devices booted", {
      encoding: "utf-8",
      timeout: 10_000,
    });
    // Look for lines containing "(Booted)"
    return out.includes("(Booted)");
  } catch {
    return false;
  }
}

/**
 * Start infrastructure (LiveKit server) and detect available platforms.
 */
export async function setup(config: OrchestratorConfig): Promise<PlatformAvailability> {
  // Start LiveKit Docker container
  livekit.startServer();
  await livekit.waitForHealthy(15_000);

  // Detect platforms
  const android = adb.isDeviceConnected();
  const ios = isIosSimBooted();
  // Desktop is available if we can assume a Vite dev server can be started
  const desktop = true;

  console.log("[orchestrator] LiveKit server started");
  console.log(`[orchestrator] Platforms: android=${android}, desktop=${desktop}, ios=${ios}`);

  return { android, desktop, ios };
}

/**
 * Stop infrastructure and clean up running apps.
 */
export async function teardown(): Promise<void> {
  // Force-stop Android app (ignore errors if no device)
  try {
    adb.forceStop();
  } catch {
    // No device connected — ignore.
  }

  // Terminate iOS sim app (ignore errors if no sim)
  try {
    execSync("xcrun simctl terminate booted io.visio.mobile", {
      timeout: 10_000,
      stdio: "pipe",
    });
  } catch {
    // No sim or app not running — ignore.
  }

  // Stop LiveKit Docker container
  livekit.stopServer();

  console.log("[orchestrator] Teardown complete");
}
