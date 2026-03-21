import { mkdirSync } from "node:fs";
import { dirname } from "node:path";
import { execSync } from "node:child_process";
import { screencap } from "../utils/adb.js";

/**
 * Create parent directories for the given path if they do not exist.
 */
export function ensureDir(dir: string): void {
  mkdirSync(dir, { recursive: true });
}

/**
 * Capture a screenshot from a connected Android device via ADB.
 * Returns the output path.
 */
export async function captureAndroid(outputPath: string): Promise<string> {
  ensureDir(dirname(outputPath));
  screencap(outputPath);
  return outputPath;
}

/**
 * Capture a screenshot from a Playwright-controlled desktop window.
 * Returns the output path.
 *
 * @param page - A Playwright Page object (typed as `any` for now to avoid
 *               a hard dependency on playwright-core types at the util level).
 */
export async function captureDesktop(page: any, outputPath: string): Promise<string> {
  ensureDir(dirname(outputPath));
  await page.screenshot({ path: outputPath });
  return outputPath;
}

/**
 * Capture a screenshot from the booted iOS Simulator.
 * Returns the output path.
 */
export async function captureIosSim(outputPath: string): Promise<string> {
  ensureDir(dirname(outputPath));
  execSync(`xcrun simctl io booted screenshot "${outputPath}"`, { timeout: 10_000 });
  return outputPath;
}
