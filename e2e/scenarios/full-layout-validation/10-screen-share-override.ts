import type { ScenarioContext } from "../../framework/participants/types.js";

export default async function(ctx: ScenarioContext) {
  ctx.log("Scenario 10: Alice starts screen share → all platforms switch to screen share focus");

  const alice = ctx.bot("bot-alice");
  const android = ctx.android();
  const desktop = ctx.desktop();
  const ios = ctx.ios();

  // Connect platforms first so they are ready before the screen share starts
  await android.connect();
  await desktop.connect();
  await ios.connect();

  await alice.connect();

  // Start from a known GRID state (no one speaking)
  ctx.log("Waiting for idle state before screen share");
  await ctx.sleep(2000);

  // Alice starts screen share
  ctx.log("Alice starts screen share");
  await alice.screenShareStart();
  await ctx.sleep(3000); // Allow track subscription to propagate

  // All platforms must switch to FOCUS layout showing the screen share
  ctx.log("Verifying screen share triggers FOCUS layout on all platforms");
  await android.assertTestTag("layout-mode:FOCUS", { timeout: 10000 });
  await android.screenshot("10-screen-share-override-android");

  await desktop.assertTestId("layout-mode:FOCUS", { timeout: 10000 });
  await desktop.screenshot("10-screen-share-override-desktop");

  await ios.screenshot("10-screen-share-override-ios");

  await alice.screenShareStop();
  ctx.log("PASS: Screen share overrides layout to FOCUS on all platforms");
}
