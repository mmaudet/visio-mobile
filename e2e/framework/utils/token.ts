import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { resolve } from "node:path";

/**
 * Locate the visio-bot binary, preferring release over debug.
 */
function findBotBinary(): string {
  const root = resolve(import.meta.dirname, "..", "..", "..");
  const release = resolve(root, "target", "release", "visio-bot");
  const debug = resolve(root, "target", "debug", "visio-bot");

  if (existsSync(release)) return release;
  if (existsSync(debug)) return debug;

  throw new Error(
    `visio-bot binary not found. Checked:\n  ${release}\n  ${debug}\nBuild it with: cargo build -p visio-bot [--release]`,
  );
}

export interface GenerateTokenOpts {
  url: string;
  room: string;
  identity: string;
  name: string;
  apiKey?: string;
  apiSecret?: string;
}

/**
 * Generate a LiveKit JWT token by invoking the visio-bot binary in --token-only mode.
 */
export function generateToken(opts: GenerateTokenOpts): string {
  const bin = findBotBinary();

  const args = [
    "--token-only",
    "--url", opts.url,
    "--room", opts.room,
    "--identity", opts.identity,
    "--name", opts.name,
  ];

  if (opts.apiKey) {
    args.push("--api-key", opts.apiKey);
  }
  if (opts.apiSecret) {
    args.push("--api-secret", opts.apiSecret);
  }

  const stdout = execFileSync(bin, args, {
    encoding: "utf-8",
    timeout: 10_000,
  });

  return stdout.trim();
}
