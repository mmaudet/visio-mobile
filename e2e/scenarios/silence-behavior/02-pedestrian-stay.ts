import type { ScenarioContext } from "../../framework/participants/types.js";

export default async function(ctx: ScenarioContext) {
  const alice = ctx.bot("bot-a");
  const android = ctx.android();

  ctx.log("Alice speaks to trigger focus");
  await alice.speak();
  await alice.waitForEvent(/ActiveSpeakers.*bot-a/, 5000);
  await ctx.sleep(2000);

  const aliceSid = alice.sid;

  // Verify focus established
  await android.assertTestTag(`main-tile:${aliceSid}`, { timeout: 5000 });
  await android.screenshot("02-pedestrian-focus");

  ctx.log("Alice mutes — waiting 6s (pedestrian should keep last speaker)");
  await alice.mute();
  await ctx.sleep(6000);

  // In pedestrian mode, should NOT return to grid
  // Note: This test assumes the device is in pedestrian mode
  // The main tile should still show Alice
  ctx.log("Verifying main tile still shows Alice");
  await android.assertTestTag(`main-tile:${aliceSid}`, { timeout: 3000 });
  await android.screenshot("02-pedestrian-still-focused");

  ctx.log("PASS: Pedestrian mode keeps last speaker after silence");
}
