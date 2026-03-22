export interface Participant {
  readonly identity: string;
  connect(): Promise<void>;
  disconnect(): Promise<void>;
  screenshot(name: string): Promise<string>;
}

export interface BotParticipant extends Participant {
  readonly sid: string;
  speak(): Promise<void>;
  mute(): Promise<void>;
  videoOn(): Promise<void>;
  videoOff(): Promise<void>;
  screenShareStart(): Promise<void>;
  screenShareStop(): Promise<void>;
  waitForEvent(pattern: string | RegExp, timeout?: number): Promise<string>;
}

export interface AndroidParticipant extends Participant {
  assertTestTag(tag: string, options?: { timeout?: number }): Promise<void>;
  assertNotTestTag(tag: string, options?: { timeout?: number }): Promise<void>;
  tap(tag: string): Promise<void>;
  longPress(tag: string): Promise<void>;
  dumpUiTree(): Promise<string>;
}

export interface DesktopParticipant extends Participant {
  assertTestId(testId: string, options?: { timeout?: number }): Promise<void>;
  assertNotTestId(testId: string, options?: { timeout?: number }): Promise<void>;
  click(testId: string): Promise<void>;
}

export interface IosSimParticipant extends Participant {
  // v1: connect + screenshot only
}

export interface ScenarioContext {
  bot(identity: string): BotParticipant;
  android(): AndroidParticipant;
  desktop(): DesktopParticipant;
  ios(): IosSimParticipant;
  sleep(ms: number): Promise<void>;
  log(message: string): void;
  sidMap: ReadonlyMap<string, string>;
}

export type ScenarioFn = (ctx: ScenarioContext) => Promise<void>;

export interface SuiteConfig {
  name: string;
  description: string;
  bots: Array<{ identity: string; name: string; mediaFile?: string }>;
  requires: {
    android?: boolean;
    desktop?: boolean;
    ios?: boolean;
  };
}

export interface AssertionResult {
  description: string;
  platform: string;
  status: "pass" | "fail";
  durationMs: number;
  error?: string;
}

export interface ScenarioResult {
  name: string;
  status: "pass" | "fail" | "skip";
  duration: number;
  assertions: AssertionResult[];
  screenshots: string[];
  error?: string;
}

export interface SuiteResult {
  name: string;
  status: "pass" | "fail" | "skip";
  scenarios: ScenarioResult[];
  skipReason?: string;
}

export interface RunResult {
  timestamp: string;
  duration: number;
  participants: {
    bots: string[];
    desktop: boolean;
    android: boolean;
    ios: boolean;
  };
  suites: SuiteResult[];
  summary: { pass: number; fail: number; skip: number; total: number };
}
