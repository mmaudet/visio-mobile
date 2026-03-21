import { execSync, execFileSync } from "node:child_process";

/**
 * Check whether an Android device (or emulator) is connected via ADB.
 */
export function isDeviceConnected(): boolean {
  try {
    const out = execFileSync("adb", ["devices"], { encoding: "utf-8" });
    const lines = out.trim().split("\n").slice(1); // skip header
    return lines.some((line) => line.includes("\tdevice"));
  } catch {
    return false;
  }
}

/**
 * Dump the current UI hierarchy via uiautomator and return the raw XML string.
 */
export function dumpUiTree(): string {
  // Dump to a temp file on the device, then read its contents.
  // /dev/tty does not work reliably on all devices (e.g. Pixel Fold).
  const dumpPath = "/sdcard/e2e-ui-dump.xml";
  execSync(`adb shell uiautomator dump ${dumpPath}`, {
    timeout: 15_000,
    stdio: "pipe",
  });
  const out = execSync(`adb shell cat ${dumpPath}`, {
    encoding: "utf-8",
    timeout: 10_000,
  });
  return out;
}

/**
 * Check if a UI element with the given testTag (resource-id) exists on screen.
 */
export function hasTestTag(tag: string): boolean {
  const xml = dumpUiTree();
  // Exact match: resource-id must be followed by a closing quote (no substring match)
  return xml.includes(`resource-id="${tag}"`);
}

/**
 * Parse the bounds attribute of a UI element identified by resource-id.
 * Returns the bounding rect (x, y, width, height) or null if not found.
 */
export function getTestTagBounds(
  tag: string,
): { x: number; y: number; width: number; height: number } | null {
  const xml = dumpUiTree();
  const needle = `resource-id="${tag}"`;
  const idx = xml.indexOf(needle);
  if (idx === -1) return null;

  // Find the enclosing <node …> element around the match.
  const nodeStart = xml.lastIndexOf("<node", idx);
  const nodeEnd = xml.indexOf(">", idx);
  if (nodeStart === -1 || nodeEnd === -1) return null;

  const nodeSnippet = xml.slice(nodeStart, nodeEnd + 1);

  // bounds="[left,top][right,bottom]"
  const boundsMatch = nodeSnippet.match(
    /bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/,
  );
  if (!boundsMatch) return null;

  const left = parseInt(boundsMatch[1], 10);
  const top = parseInt(boundsMatch[2], 10);
  const right = parseInt(boundsMatch[3], 10);
  const bottom = parseInt(boundsMatch[4], 10);

  return {
    x: left,
    y: top,
    width: right - left,
    height: bottom - top,
  };
}

/**
 * Tap at the given screen coordinates.
 */
export function tap(x: number, y: number): void {
  execFileSync("adb", ["shell", "input", "tap", String(x), String(y)]);
}

/**
 * Find the element by testTag and tap its center.
 */
export function tapTestTag(tag: string): void {
  const bounds = getTestTagBounds(tag);
  if (!bounds) {
    throw new Error(`tapTestTag: element with resource-id="${tag}" not found`);
  }
  tap(bounds.x + Math.floor(bounds.width / 2), bounds.y + Math.floor(bounds.height / 2));
}

/**
 * Long press at the given coordinates for the specified duration.
 */
export function longPress(x: number, y: number, durationMs: number = 1500): void {
  execFileSync("adb", [
    "shell", "input", "swipe",
    String(x), String(y), String(x), String(y), String(durationMs),
  ]);
}

/**
 * Find the element by testTag and long press its center.
 */
export function longPressTestTag(tag: string): void {
  const bounds = getTestTagBounds(tag);
  if (!bounds) {
    throw new Error(`longPressTestTag: element with resource-id="${tag}" not found`);
  }
  longPress(
    bounds.x + Math.floor(bounds.width / 2),
    bounds.y + Math.floor(bounds.height / 2),
  );
}

/**
 * Take a screenshot and save it to outputPath.
 */
export function screencap(outputPath: string): void {
  const { writeFileSync } = require("node:fs");
  const buf = execFileSync("adb", ["exec-out", "screencap", "-p"], {
    timeout: 10_000,
    maxBuffer: 10 * 1024 * 1024,
  });
  writeFileSync(outputPath, buf);
}

/**
 * Launch the Visio Mobile app with the given deep link URL.
 */
export function launchDeepLink(url: string): void {
  // Wrap the URL in single quotes so the Android shell does not interpret & or other metacharacters.
  // The URL must not contain single quotes itself (safe for our use case).
  execFileSync("adb", [
    "shell", `am start -a android.intent.action.VIEW -d '${url}' io.visio.mobile`,
  ], { timeout: 10_000, stdio: "pipe" });
}

/**
 * Force-stop the Visio Mobile app.
 */
export function forceStop(): void {
  execFileSync("adb", ["shell", "am", "force-stop", "io.visio.mobile"]);
}
