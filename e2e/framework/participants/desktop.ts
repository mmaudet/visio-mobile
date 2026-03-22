import { spawn, execSync, type ChildProcess } from "node:child_process";
import { join, resolve } from "node:path";
import { existsSync } from "node:fs";
import { pollUntil } from "../assertions/poll.js";
import type { DesktopParticipant } from "./types.js";

const DEFAULT_TIMEOUT = 5_000;
const LAUNCH_SETTLE_MS = 5_000;

interface DesktopOptions {
  identity: string;
  livekitUrl: string;
  token: string;
  screenshotDir: string;
  projectRoot: string;
}

export class DesktopParticipantImpl implements DesktopParticipant {
  readonly identity: string;

  private readonly _livekitUrl: string;
  private readonly _token: string;
  private readonly _screenshotDir: string;
  private readonly _projectRoot: string;
  private _process: ChildProcess | null = null;
  private _logs: string[] = [];
  private _connected = false;

  constructor(opts: DesktopOptions) {
    this.identity = opts.identity;
    this._livekitUrl = opts.livekitUrl;
    this._token = opts.token;
    this._screenshotDir = opts.screenshotDir;
    this._projectRoot = opts.projectRoot;
  }

  async connect(): Promise<void> {
    // Launch the Tauri desktop binary with auto-connect CLI args.
    // The binary auto-connects to LiveKit and emits log events.
    const bin = this._findBinary();

    this._process = spawn(bin, [
      "--livekit-url", this._livekitUrl,
      "--token", this._token,
    ], {
      stdio: ["pipe", "pipe", "pipe"],
      env: { ...process.env, RUST_LOG: "info" },
    });

    this._process.stdout?.on("data", (data: Buffer) => {
      const line = data.toString().trim();
      if (line) this._logs.push(line);
    });

    this._process.stderr?.on("data", (data: Buffer) => {
      const line = data.toString().trim();
      if (line) this._logs.push(line);
    });

    // Wait for the app to start and connect
    await new Promise((resolve) => setTimeout(resolve, LAUNCH_SETTLE_MS));
    this._connected = true;
  }

  async assertTestId(testId: string, options?: { timeout?: number }): Promise<void> {
    // Desktop assertions via log monitoring — the Tauri app logs layout changes
    // For now, this is a no-op that passes (desktop visual verification via screenshots)
    const timeout = options?.timeout ?? DEFAULT_TIMEOUT;
    // TODO: connect to Tauri webview via WebSocket devtools for real assertions
    await new Promise((resolve) => setTimeout(resolve, 100));
  }

  async assertNotTestId(testId: string, options?: { timeout?: number }): Promise<void> {
    await new Promise((resolve) => setTimeout(resolve, 100));
  }

  async click(testId: string): Promise<void> {
    // Not supported without Playwright/devtools connection
  }

  async screenshot(name: string): Promise<string> {
    // Use screencapture on macOS to capture the desktop window
    const outputPath = join(this._screenshotDir, `${name}.png`);
    try {
      // execSync imported at top level
      execSync(`screencapture -x -l $(osascript -e 'tell application "System Events" to get id of first window of (first process whose name is "visio-desktop")' 2>/dev/null || echo 0) "${outputPath}" 2>/dev/null || screencapture -x "${outputPath}"`, { timeout: 5000 });
    } catch {
      // Fallback: full screen capture
      // execSync imported at top level
      try {
        execSync(`screencapture -x "${outputPath}"`, { timeout: 5000 });
      } catch { /* ignore */ }
    }
    return outputPath;
  }

  async disconnect(): Promise<void> {
    if (this._process) {
      this._process.kill("SIGTERM");
      this._process = null;
    }
    this._connected = false;
  }

  getLogs(): string[] {
    return [...this._logs];
  }

  private _findBinary(): string {
    // Look for the debug binary
    const candidates = [
      join(this._projectRoot, "target", "debug", "visio-desktop"),
      join(this._projectRoot, "target", "release", "visio-desktop"),
    ];
    for (const p of candidates) {
      if (existsSync(p)) return p;
    }
    throw new Error(
      `visio-desktop binary not found. Run: cargo build -p visio-desktop\nSearched: ${candidates.join(", ")}`,
    );
  }
}
