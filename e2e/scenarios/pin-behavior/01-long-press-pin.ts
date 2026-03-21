import type { ScenarioContext } from "../../framework/participants/types.js";

export default async function(ctx: ScenarioContext) {
  const alice = ctx.bot("bot-a");
  const bob = ctx.bot("bot-b");
  const android = ctx.android();

  // Both bots connect, Alice speaks to appear in tiles
  ctx.log("Alice speaks to establish focus");
  await alice.speak();
  await alice.waitForEvent(/ActiveSpeakers.*bot-a/, 5000);
  await ctx.sleep(2000);

  const aliceSid = alice.sid;

  // Verify Alice is in main tile
  await android.assertTestTag(`main-tile:${aliceSid}`, { timeout: 5000 });

  // Long press on Alice's tile to pin
  ctx.log("Long pressing Alice tile to pin");
  await android.longPress(`main-tile:${aliceSid}`);
  await ctx.sleep(500);

  // Verify pin indicator appears
  await android.assertTestTag(`pin-indicator:${aliceSid}`, { timeout: 3000 });
  await android.screenshot("01-pin-indicator-visible");

  await alice.mute();
  ctx.log("PASS: Long press shows pin indicator");
}
