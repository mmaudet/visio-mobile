import type { ScenarioContext } from "../../framework/participants/types.js";

export default async function(ctx: ScenarioContext) {
  const alice = ctx.bot("bot-a");
  const android = ctx.android();
  const desktop = ctx.desktop();

  ctx.log("Alice speaks to trigger focus mode");
  await alice.speak();
  await alice.waitForEvent(/ActiveSpeakers.*bot-a/, 5000);
  await ctx.sleep(2000);

  await android.assertTestTag("layout-mode:FOCUS", { timeout: 5000 });
  await android.screenshot("01-focus-while-speaking");

  ctx.log("Alice mutes — waiting 6s for silence timeout");
  await alice.mute();
  await ctx.sleep(6000);

  ctx.log("Verifying grid mode after silence");
  await android.assertTestTag("layout-mode:GRID", { timeout: 5000 });
  await android.screenshot("01-grid-after-silence");

  await desktop.assertTestId("layout-mode:GRID", { timeout: 5000 });
  await desktop.screenshot("01-grid-after-silence");

  ctx.log("PASS: Office mode returns to grid after 5s silence");
}
