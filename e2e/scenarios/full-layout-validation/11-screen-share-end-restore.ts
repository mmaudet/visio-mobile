import type { ScenarioContext } from "../../framework/participants/types.js";

export default async function(ctx: ScenarioContext) {
  ctx.log("Scenario 11: Alice stops screen share → layout returns to previous state (GRID)");

  const alice = ctx.bot("bot-alice");
  const android = ctx.android();
  const desktop = ctx.desktop();
  const ios = ctx.ios();

  await alice.connect();
  if (android) await android.connect();
  if (desktop) await desktop.connect();
  if (ios) await ios.connect();

  // Verify initial GRID state (no one speaking, no screen share)
  ctx.log("Verifying initial GRID state");
  await ctx.sleep(2000);

  if (android) {
    await android.assertTestTag("layout-mode:GRID", { timeout: 5000 });
    await android.screenshot("11-initial-grid-android");
  }

  // Alice starts screen share → layout switches to FOCUS
  ctx.log("Alice starts screen share");
  await alice.screenShareStart();
  await ctx.sleep(3000);

  if (android) {
    await android.assertTestTag("layout-mode:FOCUS", { timeout: 5000 });
    await android.screenshot("11-screen-share-active-android");
  }

  if (desktop) {
    await desktop.assertTestId("layout-mode:FOCUS", { timeout: 5000 });
    await desktop.screenshot("11-screen-share-active-desktop");
  }

  if (ios) await ios.screenshot("11-screen-share-active-ios");

  // Alice stops screen share → layout should return to GRID
  ctx.log("Alice stops screen share — verifying GRID is restored");
  await alice.screenShareStop();
  await ctx.sleep(3000);

  if (android) {
    await android.assertTestTag("layout-mode:GRID", { timeout: 5000 });
    await android.screenshot("11-grid-restored-android");
  }

  if (desktop) {
    await desktop.assertTestId("layout-mode:GRID", { timeout: 5000 });
    await desktop.screenshot("11-grid-restored-desktop");
  }

  if (ios) await ios.screenshot("11-grid-restored-ios");

  ctx.log("PASS: Layout correctly restored to GRID after screen share ends");
}
