import type { ScenarioContext } from "../../framework/participants/types.js";

export default async function(ctx: ScenarioContext) {
  ctx.log("Scenario 05: Silence + Office mode — all speak then mute → grid returns after 5s silence");

  const alice = ctx.bot("bot-alice");
  const bob = ctx.bot("bot-bob");
  const android = ctx.android();
  const desktop = ctx.desktop();
  const ios = ctx.ios();

  await alice.connect();
  await bob.connect();
  if (android) await android.connect();
  if (desktop) await desktop.connect();
  if (ios) await ios.connect();

  // Alice speaks to enter FOCUS mode
  ctx.log("Alice speaks to establish FOCUS mode");
  await alice.speak();
  await alice.waitForEvent(/ActiveSpeakers.*bot-alice/, 5000);
  await ctx.sleep(2000);

  if (android) {
    await android.assertTestTag("layout-mode:FOCUS", { timeout: 5000 });
    await android.screenshot("05-focus-while-speaking-android");
  }

  // Bob joins the conversation briefly
  ctx.log("Bob speaks briefly then both mute");
  await bob.speak();
  await ctx.sleep(1000);
  await alice.mute();
  await bob.mute();

  // Wait for silence timeout (> 5s)
  ctx.log("All muted — waiting 6s for silence-to-grid timeout");
  await ctx.sleep(6000);

  // Office mode should return to GRID
  ctx.log("Verifying grid mode returned after silence");
  if (android) {
    await android.assertTestTag("layout-mode:GRID", { timeout: 5000 });
    await android.screenshot("05-grid-after-silence-android");
  }

  if (desktop) {
    await desktop.assertTestId("layout-mode:GRID", { timeout: 5000 });
    await desktop.screenshot("05-grid-after-silence-desktop");
  }

  if (ios) await ios.screenshot("05-grid-after-silence-ios");

  ctx.log("PASS: Office mode returns to GRID layout after 5s of silence");
}
