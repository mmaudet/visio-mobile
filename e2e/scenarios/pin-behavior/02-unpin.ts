import type { ScenarioContext } from "../../framework/participants/types.js";

export default async function(ctx: ScenarioContext) {
  const alice = ctx.bot("bot-a");
  const bob = ctx.bot("bot-b");
  const android = ctx.android();

  // Setup: Alice speaks, pin her
  await alice.speak();
  await alice.waitForEvent(/ActiveSpeakers.*bot-a/, 5000);
  await ctx.sleep(2000);

  const aliceSid = alice.sid;
  await android.assertTestTag(`main-tile:${aliceSid}`, { timeout: 5000 });

  // Pin Alice
  ctx.log("Pinning Alice");
  await android.longPress(`main-tile:${aliceSid}`);
  await ctx.sleep(500);
  await android.assertTestTag(`pin-indicator:${aliceSid}`, { timeout: 3000 });

  // Unpin Alice (long press again)
  ctx.log("Long pressing again to unpin");
  await android.longPress(`main-tile:${aliceSid}`);
  await ctx.sleep(500);

  // Verify pin indicator is gone
  await android.assertNotTestTag(`pin-indicator:${aliceSid}`, { timeout: 3000 });
  await android.screenshot("02-pin-removed");

  await alice.mute();
  ctx.log("PASS: Second long press removes pin");
}
