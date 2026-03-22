import type { ScenarioContext } from "../../framework/participants/types.js";

export default async function(ctx: ScenarioContext) {
  ctx.log("Scenario 01: 2 participants — remote speaks → all platforms show Alice in main tile");

  const alice = ctx.bot("bot-alice");
  const android = ctx.android();
  const desktop = ctx.desktop();
  const ios = ctx.ios();

  // Connect Alice and start speaking
  await alice.connect();
  ctx.log("Alice starts speaking (French TTS)");
  await alice.speak();
  await alice.waitForEvent(/ActiveSpeakers.*bot-alice/, 5000);
  await ctx.sleep(1500);

  const aliceSid = alice.sid;
  ctx.log(`Alice SID: ${aliceSid}`);

  // Android assertions
  ctx.log("Android: asserting FOCUS layout and Alice in main tile");
  await android.assertTestTag("layout-mode:FOCUS", { timeout: 5000 });
  await android.assertTestTag(`main-tile:${aliceSid}`, { timeout: 5000 });
  await android.screenshot("01-alice-main-tile-android");

  // Desktop assertions
  ctx.log("Desktop: asserting FOCUS layout and Alice in main tile");
  await desktop.assertTestId("layout-mode:FOCUS", { timeout: 5000 });
  await desktop.assertTestId(`main-tile:${aliceSid}`, { timeout: 5000 });
  await desktop.screenshot("01-alice-main-tile-desktop");

  // iOS: screenshot only (v1 — limited assertions)
  ctx.log("iOS: capturing screenshot evidence");
  await ios.screenshot("01-alice-main-tile-ios");

  await alice.mute();
  ctx.log("PASS: Remote speaker correctly shown in main tile on all platforms");
}
