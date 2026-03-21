import type { ScenarioContext } from "../../framework/participants/types.js";

export default async function(ctx: ScenarioContext) {
  const alice = ctx.bot("bot-a");
  const android = ctx.android();
  const desktop = ctx.desktop();

  // Alice starts screen share
  ctx.log("Alice starts screen share");
  await alice.screenShareStart();
  await ctx.sleep(3000); // Wait for track subscription

  ctx.log("Verifying screen share in main tile");
  await android.assertTestTag("layout-mode:FOCUS", { timeout: 5000 });
  await android.screenshot("01-screen-share-override");

  await desktop.assertTestId("layout-mode:FOCUS", { timeout: 5000 });
  await desktop.screenshot("01-screen-share-override");

  await alice.screenShareStop();
  ctx.log("PASS: Screen share overrides layout to focus mode");
}
