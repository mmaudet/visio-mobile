import type { ScenarioContext } from "../../framework/participants/types.js";

export default async function(ctx: ScenarioContext) {
  const alice = ctx.bot("bot-a");
  const android = ctx.android();
  const desktop = ctx.desktop();

  // First verify we start in grid mode
  await ctx.sleep(2000);
  await android.assertTestTag("layout-mode:GRID", { timeout: 5000 });
  await android.screenshot("02-initial-grid");

  // Alice starts screen share
  ctx.log("Alice starts screen share");
  await alice.screenShareStart();
  await ctx.sleep(3000);

  await android.assertTestTag("layout-mode:FOCUS", { timeout: 5000 });
  await android.screenshot("02-screen-share-active");

  // Alice stops screen share
  ctx.log("Alice stops screen share — should return to grid");
  await alice.screenShareStop();
  await ctx.sleep(3000);

  await android.assertTestTag("layout-mode:GRID", { timeout: 5000 });
  await android.screenshot("02-grid-restored");

  await desktop.assertTestId("layout-mode:GRID", { timeout: 5000 });
  await desktop.screenshot("02-grid-restored");

  ctx.log("PASS: Layout restored after screen share ends");
}
