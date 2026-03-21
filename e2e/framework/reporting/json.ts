import { writeFileSync, mkdirSync } from "node:fs";
import { join } from "node:path";
import type { RunResult } from "../participants/types.js";

/**
 * Write the run result as a JSON file to the report directory.
 */
export function writeJsonReport(result: RunResult, reportDir: string): void {
  mkdirSync(reportDir, { recursive: true });
  const outputPath = join(reportDir, "results.json");
  writeFileSync(outputPath, JSON.stringify(result, null, 2), "utf-8");
}
