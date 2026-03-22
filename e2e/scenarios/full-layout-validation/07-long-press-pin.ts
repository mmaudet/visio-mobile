import type { ScenarioContext } from "../../framework/participants/types.js";

export default async function(ctx: ScenarioContext) {
  ctx.log("Scenario 07: Mobile only — long press on Alice's tile → pin indicator appears");

  const alice = ctx.bot("bot-alice");
  const android = ctx.android();
  const ios = ctx.ios();

  // Connect platforms first so they are ready before the bot speaks
  await android.connect();
  await ios.connect();

  await alice.connect();

  // Alice speaks so her tile is visible and in main position
  ctx.log("Alice speaks to establish focus");
  await alice.speak();
  await alice.waitForEvent(/ActiveSpeakers.*bot-alice/, 10000);
  await ctx.sleep(2000);

  const aliceSid = alice.sid;

  // Verify Alice is in main tile before pinning
  await android.assertTestTag(`main-tile:${aliceSid}`, { timeout: 10000 });

  // Long press on Alice's tile to pin her
  ctx.log("Long pressing Alice's main tile to pin");
  await android.longPress(`main-tile:${aliceSid}`);
  await ctx.sleep(500);

  // Pin indicator must appear on Alice's tile
  await android.assertTestTag(`pin-indicator:${aliceSid}`, { timeout: 10000 });
  await android.screenshot("07-pin-indicator-visible-android");

  // iOS: screenshot evidence of pin UI
  ctx.log("iOS: capturing pin indicator screenshot evidence");
  await ios.screenshot("07-pin-indicator-visible-ios");

  await alice.mute();
  ctx.log("PASS: Long press shows pin indicator on Android; iOS screenshot captured");
}
