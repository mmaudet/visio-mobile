import { resolve, join } from "node:path";
import { mkdirSync } from "node:fs";
import { setup, teardown, getConfig } from "./orchestrator.js";
import { discoverSuites, runSuites } from "./runner.js";
import { printScenarioResult, printSummary } from "./reporting/terminal.js";
import { writeJsonReport } from "./reporting/json.js";
import { writeHtmlReport } from "./reporting/html.js";

const args = process.argv.slice(2);
const command = args[0];
const filters = args.slice(1);

function makeReportDir(): string {
  const ts = new Date()
    .toISOString()
    .replace(/:/g, "-")
    .replace(/\.\d+Z$/, "");
  const dir = resolve(import.meta.dirname, "..", "reports", ts);
  mkdirSync(dir, { recursive: true });
  return dir;
}

async function listCommand(): Promise<void> {
  const scenariosDir = resolve(import.meta.dirname, "..", "scenarios");
  const suites = discoverSuites(scenariosDir);

  if (suites.length === 0) {
    console.log("No suites found in e2e/scenarios/");
    return;
  }

  console.log("Available suites:\n");
  for (const suite of suites) {
    const requires = Object.entries(suite.config.requires)
      .filter(([, v]) => v)
      .map(([k]) => k)
      .join(", ");
    const reqStr = requires ? ` [requires: ${requires}]` : "";
    console.log(`  ${suite.config.name}${reqStr}`);
    console.log(`    ${suite.config.description}`);
    console.log(`    Scenarios: ${suite.scenarioFiles.length}`);
    console.log(`    Bots: ${suite.config.bots.map((b) => b.identity).join(", ")}`);
    console.log("");
  }
}

async function runCommand(): Promise<void> {
  const config = getConfig();
  const reportDir = makeReportDir();

  console.log("[cli] Starting E2E run...\n");

  let availability;
  try {
    availability = await setup(config);
  } catch (err) {
    console.error("[cli] Setup failed:", err);
    process.exit(1);
  }

  let result;
  try {
    result = await runSuites(filters, availability, config, reportDir);

    // Print terminal results
    for (const suite of result.suites) {
      for (const scenario of suite.scenarios) {
        printScenarioResult(suite.name, scenario);
      }
    }

    // Write reports
    writeJsonReport(result, reportDir);
    writeHtmlReport(result, reportDir);
    printSummary(result, reportDir);
  } finally {
    await teardown();
  }

  // Exit with appropriate code
  process.exit(result.summary.fail > 0 ? 1 : 0);
}

// ── Main ──

switch (command) {
  case "list":
    await listCommand();
    break;
  case "run":
    await runCommand();
    break;
  default:
    console.log("Usage:");
    console.log("  npx tsx framework/cli.ts run [suite1] [suite2] [suite/scenario]");
    console.log("  npx tsx framework/cli.ts list");
    process.exit(command ? 1 : 0);
}
