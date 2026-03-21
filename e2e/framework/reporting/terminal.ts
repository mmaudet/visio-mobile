import type { ScenarioResult, RunResult } from "../participants/types.js";

// ANSI color codes
const GREEN = "\x1b[32m";
const RED = "\x1b[31m";
const YELLOW = "\x1b[33m";
const DIM = "\x1b[2m";
const BOLD = "\x1b[1m";
const RESET = "\x1b[0m";

function statusColor(status: "pass" | "fail" | "skip"): string {
  switch (status) {
    case "pass":
      return GREEN;
    case "fail":
      return RED;
    case "skip":
      return YELLOW;
  }
}

function statusLabel(status: "pass" | "fail" | "skip"): string {
  return status.toUpperCase();
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

/**
 * Print a single scenario result to the terminal.
 */
export function printScenarioResult(suite: string, scenario: ScenarioResult): void {
  const color = statusColor(scenario.status);
  const label = statusLabel(scenario.status);
  const duration = formatDuration(scenario.duration);

  // Pad the name + dots to a fixed width for alignment
  const prefix = `  [${suite}] ${scenario.name} `;
  const dots = ".".repeat(Math.max(2, 60 - prefix.length));

  console.log(
    `${prefix}${DIM}${dots}${RESET} ${color}${label}${RESET} ${DIM}(${duration})${RESET}`,
  );

  if (scenario.error) {
    console.log(`    ${RED}Error: ${scenario.error}${RESET}`);
  }
}

/**
 * Print a summary of the entire run.
 */
export function printSummary(result: RunResult, reportDir: string): void {
  const { pass, fail, skip } = result.summary;

  console.log("");
  console.log(`${BOLD}Results:${RESET}`);

  const parts: string[] = [];
  if (pass > 0) parts.push(`${GREEN}${pass} passed${RESET}`);
  if (fail > 0) parts.push(`${RED}${fail} failed${RESET}`);
  if (skip > 0) parts.push(`${YELLOW}${skip} skipped${RESET}`);

  console.log(`  ${parts.join(", ")}`);
  console.log(`  Duration: ${formatDuration(result.duration)}`);

  if (reportDir) {
    console.log(`  Report: ${reportDir}/report.html`);
  }

  console.log("");
}
