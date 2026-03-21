import type { ScenarioContext } from "../../framework/participants/types.js";

export default async function(ctx: ScenarioContext) {
  const alice = ctx.bot("bot-a");
  const android = ctx.android();
  const desktop = ctx.desktop();

  // Alice speaks first to establish focus
  ctx.log("Alice speaks to establish focus");
  await alice.speak();
  await alice.waitForEvent(/ActiveSpeakers.*bot-a/, 5000);
  await ctx.sleep(2000);

  const aliceSid = alice.sid;

  // Verify Alice is in main tile
  await android.assertTestTag(`main-tile:${aliceSid}`, { timeout: 5000 });

  // Alice mutes — main tile should still show Alice (not switch to self)
  ctx.log("Alice mutes — verifying main tile stays on Alice");
  await alice.mute();
  await ctx.sleep(2000); // Wait but less than silence-to-grid timeout (5s)

  // Main tile should still be Alice (no self-focus)
  await android.assertTestTag(`main-tile:${aliceSid}`, { timeout: 3000 });
  await android.screenshot("02-alice-muted-still-main");

  await desktop.assertTestId(`main-tile:${aliceSid}`, { timeout: 3000 });
  await desktop.screenshot("02-alice-muted-still-main");

  ctx.log("PASS: Local speaker does not trigger self-focus");
}
