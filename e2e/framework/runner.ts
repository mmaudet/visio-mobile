import { readdirSync, readFileSync, existsSync } from "node:fs";
import { resolve, join, basename } from "node:path";
import { generateToken } from "./utils/token.js";
import { BotParticipantImpl } from "./participants/bot.js";
import { AndroidParticipantImpl } from "./participants/android.js";
import { DesktopParticipantImpl } from "./participants/desktop.js";
import { IosSimParticipantImpl } from "./participants/ios-sim.js";
import type { PlatformAvailability, OrchestratorConfig } from "./orchestrator.js";
import type {
  BotParticipant,
  AndroidParticipant,
  DesktopParticipant,
  IosSimParticipant,
  ScenarioContext,
  ScenarioFn,
  SuiteConfig,
  ScenarioResult,
  SuiteResult,
  RunResult,
} from "./participants/types.js";

const DEFAULT_SCENARIO_TIMEOUT = parseInt(
  process.env.E2E_SCENARIO_TIMEOUT ?? "60000",
  10,
);

// ── Suite discovery ──

interface DiscoveredSuite {
  config: SuiteConfig;
  dir: string;
  scenarioFiles: string[];
}

/**
 * Scan e2e/scenarios/ for suite.json files and return discovered suites.
 */
export function discoverSuites(scenariosDir: string): DiscoveredSuite[] {
  if (!existsSync(scenariosDir)) return [];

  const suites: DiscoveredSuite[] = [];
  const entries = readdirSync(scenariosDir, { withFileTypes: true });

  for (const entry of entries) {
    if (!entry.isDirectory()) continue;

    const suiteDir = join(scenariosDir, entry.name);
    const suiteJsonPath = join(suiteDir, "suite.json");

    if (!existsSync(suiteJsonPath)) continue;

    const config: SuiteConfig = JSON.parse(
      readFileSync(suiteJsonPath, "utf-8"),
    );

    // Find scenario .ts files (anything that isn't suite.json)
    const files = readdirSync(suiteDir)
      .filter((f) => f.endsWith(".ts"))
      .sort()
      .map((f) => join(suiteDir, f));

    suites.push({ config, dir: suiteDir, scenarioFiles: files });
  }

  return suites;
}

/**
 * Filter suites by CLI arguments. Supports:
 * - suite name (e.g., "speaker-focus")
 * - suite/scenario (e.g., "speaker-focus/01-remote-speaks")
 */
function filterSuites(
  suites: DiscoveredSuite[],
  filters: string[],
): { suite: DiscoveredSuite; scenarioFilter: string | null }[] {
  if (filters.length === 0) {
    return suites.map((suite) => ({ suite, scenarioFilter: null }));
  }

  const result: { suite: DiscoveredSuite; scenarioFilter: string | null }[] = [];

  for (const filter of filters) {
    const parts = filter.split("/");
    const suiteName = parts[0];
    const scenarioName = parts[1] ?? null;

    const matched = suites.find((s) => s.config.name === suiteName);
    if (matched) {
      result.push({ suite: matched, scenarioFilter: scenarioName });
    }
  }

  return result;
}

// ── ScenarioContext implementation ──

class ScenarioContextImpl implements ScenarioContext {
  private bots: Map<string, BotParticipantImpl> = new Map();
  private _android: AndroidParticipantImpl | null = null;
  private _desktop: DesktopParticipantImpl | null = null;
  private _ios: IosSimParticipantImpl | null = null;
  sidMap: Map<string, string> = new Map();

  private readonly _suiteConfig: SuiteConfig;
  private readonly _config: OrchestratorConfig;
  private readonly _screenshotDir: string;
  private readonly _availability: PlatformAvailability;

  constructor(
    suiteConfig: SuiteConfig,
    config: OrchestratorConfig,
    screenshotDir: string,
    availability: PlatformAvailability,
  ) {
    this._suiteConfig = suiteConfig;
    this._config = config;
    this._screenshotDir = screenshotDir;
    this._availability = availability;
  }

  bot(identity: string): BotParticipant {
    const existing = this.bots.get(identity);
    if (existing) return existing;

    // Verify identity is declared in suite config
    const botDef = this._suiteConfig.bots.find((b) => b.identity === identity);
    if (!botDef) {
      throw new Error(
        `Bot "${identity}" not declared in suite.json. Declared bots: ${this._suiteConfig.bots.map((b) => b.identity).join(", ")}`,
      );
    }

    const token = generateToken({
      url: this._config.livekitUrl,
      room: this._config.room,
      identity: botDef.identity,
      name: botDef.name,
      apiKey: this._config.apiKey,
      apiSecret: this._config.apiSecret,
    });

    const bot = new BotParticipantImpl({
      identity: botDef.identity,
      name: botDef.name,
      livekitUrl: this._config.livekitUrl,
      token,
      projectRoot: this._config.projectRoot,
      mediaFile: botDef.mediaFile
        ? resolve(this._config.projectRoot, "e2e", botDef.mediaFile)
        : undefined,
    });

    this.bots.set(identity, bot);
    return bot;
  }

  android(): AndroidParticipant {
    if (!this._availability.android) {
      throw new Error("Android device not available");
    }
    if (this._android) return this._android;

    const token = generateToken({
      url: this._config.livekitUrl,
      room: this._config.room,
      identity: "android-user",
      name: "Android User",
      apiKey: this._config.apiKey,
      apiSecret: this._config.apiSecret,
    });

    this._android = new AndroidParticipantImpl({
      identity: "android-user",
      livekitUrl: this._config.livekitUrl,
      token,
      screenshotDir: this._screenshotDir,
    });

    return this._android;
  }

  desktop(): DesktopParticipant {
    if (!this._availability.desktop) {
      throw new Error("Desktop not available");
    }
    if (this._desktop) return this._desktop;

    const token = generateToken({
      url: this._config.livekitUrl,
      room: this._config.room,
      identity: "desktop-user",
      name: "Desktop User",
      apiKey: this._config.apiKey,
      apiSecret: this._config.apiSecret,
    });

    this._desktop = new DesktopParticipantImpl({
      identity: "desktop-user",
      livekitUrl: this._config.livekitUrl,
      token,
      screenshotDir: this._screenshotDir,
    });

    return this._desktop;
  }

  ios(): IosSimParticipant {
    if (!this._availability.ios) {
      throw new Error("iOS Simulator not available");
    }
    if (this._ios) return this._ios;

    const token = generateToken({
      url: this._config.livekitUrl,
      room: this._config.room,
      identity: "ios-user",
      name: "iOS User",
      apiKey: this._config.apiKey,
      apiSecret: this._config.apiSecret,
    });

    this._ios = new IosSimParticipantImpl({
      identity: "ios-user",
      livekitUrl: this._config.livekitUrl,
      token,
      screenshotDir: this._screenshotDir,
    });

    return this._ios;
  }

  sleep(ms: number): Promise<void> {
    return new Promise((r) => setTimeout(r, ms));
  }

  log(message: string): void {
    console.log(`  [LOG] ${message}`);
  }

  /**
   * Disconnect all participants created during the scenario.
   */
  async disconnectAll(): Promise<void> {
    for (const bot of this.bots.values()) {
      await bot.disconnect();
    }
    if (this._android) await this._android.disconnect();
    if (this._desktop) await this._desktop.disconnect();
    if (this._ios) await this._ios.disconnect();
  }
}

// ── Scenario execution ──

async function runScenario(
  scenarioFile: string,
  ctx: ScenarioContextImpl,
  timeout: number,
): Promise<ScenarioResult> {
  const name = basename(scenarioFile, ".ts");
  const start = Date.now();
  const screenshots: string[] = [];

  try {
    const mod = await import(scenarioFile);
    const scenarioFn: ScenarioFn = mod.default ?? mod.scenario;

    if (typeof scenarioFn !== "function") {
      throw new Error(
        `Scenario file ${scenarioFile} must export a default function or a named "scenario" function`,
      );
    }

    // Run with timeout
    await Promise.race([
      scenarioFn(ctx),
      new Promise<never>((_, reject) =>
        setTimeout(
          () => reject(new Error(`Scenario timed out after ${timeout}ms`)),
          timeout,
        ),
      ),
    ]);

    return {
      name,
      status: "pass",
      duration: Date.now() - start,
      assertions: [],
      screenshots,
    };
  } catch (err) {
    const error = err instanceof Error ? err.message : String(err);
    return {
      name,
      status: "fail",
      duration: Date.now() - start,
      assertions: [],
      screenshots,
      error,
    };
  }
}

async function runSuite(
  discovered: DiscoveredSuite,
  scenarioFilter: string | null,
  availability: PlatformAvailability,
  config: OrchestratorConfig,
  reportDir: string,
): Promise<SuiteResult> {
  const { config: suiteConfig, scenarioFiles } = discovered;

  // Check platform requirements
  const requires = suiteConfig.requires;
  if (requires.android && !availability.android) {
    return {
      name: suiteConfig.name,
      status: "skip",
      scenarios: [],
      skipReason: "Android device not available",
    };
  }
  if (requires.ios && !availability.ios) {
    return {
      name: suiteConfig.name,
      status: "skip",
      scenarios: [],
      skipReason: "iOS Simulator not available",
    };
  }
  if (requires.desktop && !availability.desktop) {
    return {
      name: suiteConfig.name,
      status: "skip",
      scenarios: [],
      skipReason: "Desktop not available",
    };
  }

  // Filter scenarios if requested
  let filesToRun = scenarioFiles;
  if (scenarioFilter) {
    filesToRun = scenarioFiles.filter((f) => {
      const name = basename(f, ".ts");
      return name === scenarioFilter;
    });
  }

  const screenshotDir = join(reportDir, "screenshots", suiteConfig.name);
  const scenarios: ScenarioResult[] = [];

  for (const file of filesToRun) {
    const ctx = new ScenarioContextImpl(
      suiteConfig,
      config,
      screenshotDir,
      availability,
    );

    try {
      const result = await runScenario(file, ctx, DEFAULT_SCENARIO_TIMEOUT);
      scenarios.push(result);
    } finally {
      await ctx.disconnectAll();
    }
  }

  const hasFail = scenarios.some((s) => s.status === "fail");
  return {
    name: suiteConfig.name,
    status: hasFail ? "fail" : "pass",
    scenarios,
  };
}

// ── Public API ──

export async function runSuites(
  suiteFilter: string[],
  availability: PlatformAvailability,
  config: OrchestratorConfig,
  reportDir: string,
): Promise<RunResult> {
  const scenariosDir = resolve(import.meta.dirname, "..", "scenarios");
  const discovered = discoverSuites(scenariosDir);
  const filtered = filterSuites(discovered, suiteFilter);

  const start = Date.now();
  const suiteResults: SuiteResult[] = [];

  for (const { suite, scenarioFilter } of filtered) {
    const result = await runSuite(
      suite,
      scenarioFilter,
      availability,
      config,
      reportDir,
    );
    suiteResults.push(result);
  }

  // Collect all bot identities across all suites
  const allBots = new Set<string>();
  for (const suite of discovered) {
    for (const bot of suite.config.bots) {
      allBots.add(bot.identity);
    }
  }

  // Compute summary
  let pass = 0;
  let fail = 0;
  let skip = 0;
  for (const suite of suiteResults) {
    if (suite.status === "skip") {
      skip++; // Count skipped suite as one skip (no scenarios ran)
      continue;
    }
    for (const scenario of suite.scenarios) {
      if (scenario.status === "pass") pass++;
      else if (scenario.status === "fail") fail++;
      else skip++;
    }
  }

  return {
    timestamp: new Date().toISOString(),
    duration: Date.now() - start,
    participants: {
      bots: [...allBots],
      desktop: availability.desktop,
      android: availability.android,
      ios: availability.ios,
    },
    suites: suiteResults,
    summary: { pass, fail, skip, total: pass + fail + skip },
  };
}
