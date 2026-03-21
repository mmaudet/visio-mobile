import { spawn, type ChildProcess } from "node:child_process";
import { createInterface, type Interface as ReadlineInterface } from "node:readline";
import { existsSync } from "node:fs";
import { resolve } from "node:path";
import type { BotParticipant } from "./types.js";

const DEFAULT_ACK_TIMEOUT = 10_000;
const DEFAULT_EVENT_TIMEOUT = 15_000;
const QUIT_TIMEOUT = 5_000;

interface BotOptions {
  identity: string;
  name: string;
  livekitUrl: string;
  token: string;
  projectRoot: string;
}

/**
 * Find the visio-bot binary, preferring release over debug.
 */
function findBotBinary(projectRoot: string): string {
  const release = resolve(projectRoot, "target", "release", "visio-bot");
  const debug = resolve(projectRoot, "target", "debug", "visio-bot");

  if (existsSync(release)) return release;
  if (existsSync(debug)) return debug;

  throw new Error(
    `visio-bot binary not found. Checked:\n  ${release}\n  ${debug}\nBuild it with: cargo build -p visio-bot [--release]`,
  );
}

export class BotParticipantImpl implements BotParticipant {
  readonly identity: string;

  private _sid = "";
  private readonly _name: string;
  private readonly _livekitUrl: string;
  private readonly _token: string;
  private readonly _projectRoot: string;
  private _child: ChildProcess | null = null;
  private _rl: ReadlineInterface | null = null;
  private _lines: string[] = [];
  private _lineListeners: Array<(line: string) => void> = [];
  private static readonly MAX_LINE_BUFFER = 1000;
  private _connected = false;
  private _connecting: Promise<void> | null = null;

  get sid(): string {
    return this._sid;
  }

  constructor(opts: BotOptions) {
    this.identity = opts.identity;
    this._name = opts.name;
    this._livekitUrl = opts.livekitUrl;
    this._token = opts.token;
    this._projectRoot = opts.projectRoot;
  }

  async connect(): Promise<void> {
    const bin = findBotBinary(this._projectRoot);

    const child = spawn(bin, [
      "--interactive",
      "--url", this._livekitUrl,
      "--token", this._token,
      "--identity", this.identity,
      "--name", this._name,
    ], {
      stdio: ["pipe", "pipe", "pipe"],
    });

    this._child = child;

    // Capture stderr for diagnostics
    let stderrBuf = "";
    child.stderr?.on("data", (chunk: Buffer) => {
      stderrBuf += chunk.toString();
    });

    // Detect early exit
    child.on("exit", (code, signal) => {
      if (code !== 0 && code !== null) {
        console.error(`[bot:${this.identity}] Process exited with code ${code}`);
        if (stderrBuf) {
          const lastLines = stderrBuf.trim().split("\n").slice(-5).join("\n");
          console.error(`[bot:${this.identity}] stderr (last 5 lines):\n${lastLines}`);
        }
      }
    });

    const rl = createInterface({ input: child.stdout! });
    this._rl = rl;

    rl.on("line", (line: string) => {
      this._lines.push(line);
      // Cap buffer to prevent unbounded growth
      if (this._lines.length > BotParticipantImpl.MAX_LINE_BUFFER) {
        this._lines.splice(0, this._lines.length - BotParticipantImpl.MAX_LINE_BUFFER);
      }
      // Notify all pending listeners
      for (const listener of this._lineListeners) {
        listener(line);
      }
    });

    // Wait for the [CONNECTED] line
    let connectedLine: string;
    try {
      connectedLine = await this._waitForLine(
        (line) => line.startsWith("[CONNECTED]"),
        30_000,
        "Waiting for bot to connect",
      );
    } catch (err) {
      // Kill the child process on connect failure
      child.kill("SIGKILL");
      this._child = null;
      this._rl?.close();
      this._rl = null;
      throw err;
    }

    const sidMatch = connectedLine.match(/sid=(\S+)/);
    if (sidMatch) {
      this._sid = sidMatch[1];
    }
    this._connected = true;
  }

  private async _ensureConnected(): Promise<void> {
    if (this._connected) return;
    if (this._connecting) {
      await this._connecting;
      return;
    }
    this._connecting = this.connect();
    try {
      await this._connecting;
    } finally {
      this._connecting = null;
    }
  }

  async speak(): Promise<void> {
    await this._sendCommand("SPEAK");
  }

  async mute(): Promise<void> {
    await this._sendCommand("MUTE");
  }

  async videoOn(): Promise<void> {
    await this._sendCommand("VIDEO_ON");
  }

  async videoOff(): Promise<void> {
    await this._sendCommand("VIDEO_OFF");
  }

  async screenShareStart(): Promise<void> {
    await this._sendCommand("SCREEN_SHARE_START");
  }

  async screenShareStop(): Promise<void> {
    await this._sendCommand("SCREEN_SHARE_STOP");
  }

  async waitForEvent(pattern: string | RegExp, timeout?: number): Promise<string> {
    await this._ensureConnected();
    const timeoutMs = timeout ?? DEFAULT_EVENT_TIMEOUT;

    // First, scan already-buffered lines
    for (const line of this._lines) {
      if (matchesPattern(line, pattern)) {
        return line;
      }
    }

    // Wait for new lines
    return this._waitForLine(
      (line) => matchesPattern(line, pattern),
      timeoutMs,
      `Waiting for event matching ${pattern}`,
    );
  }

  async disconnect(): Promise<void> {
    if (!this._child) return;

    const child = this._child;

    // Write QUIT command
    if (child.stdin && !child.stdin.destroyed) {
      child.stdin.write("QUIT\n");
    }

    // Wait for exit with timeout
    await new Promise<void>((resolve) => {
      const timer = setTimeout(() => {
        child.kill("SIGKILL");
        resolve();
      }, QUIT_TIMEOUT);

      child.once("exit", () => {
        clearTimeout(timer);
        resolve();
      });
    });

    this._rl?.close();
    this._rl = null;
    this._child = null;
    this._lines = [];
    this._lineListeners = [];
  }

  async screenshot(_name: string): Promise<string> {
    // Bot participants have no visual output to screenshot.
    return "";
  }

  // ── Private helpers ──

  private async _sendCommand(cmd: string): Promise<void> {
    await this._ensureConnected();
    if (!this._child?.stdin || this._child.stdin.destroyed) {
      throw new Error(`Cannot send command "${cmd}": bot process not running`);
    }

    this._child.stdin.write(`${cmd}\n`);

    await this._waitForLine(
      (line) => line === `[ACK] ${cmd}`,
      DEFAULT_ACK_TIMEOUT,
      `Waiting for ACK of ${cmd}`,
    );
  }

  private _waitForLine(
    predicate: (line: string) => boolean,
    timeoutMs: number,
    description: string,
  ): Promise<string> {
    return new Promise<string>((resolve, reject) => {
      const timer = setTimeout(() => {
        cleanup();
        reject(new Error(`Timeout (${timeoutMs}ms): ${description}`));
      }, timeoutMs);

      const listener = (line: string) => {
        if (predicate(line)) {
          cleanup();
          resolve(line);
        }
      };

      const cleanup = () => {
        clearTimeout(timer);
        const idx = this._lineListeners.indexOf(listener);
        if (idx !== -1) this._lineListeners.splice(idx, 1);
      };

      this._lineListeners.push(listener);
    });
  }
}

function matchesPattern(line: string, pattern: string | RegExp): boolean {
  if (typeof pattern === "string") {
    return line.indexOf(pattern) !== -1;
  }
  return pattern.test(line);
}
