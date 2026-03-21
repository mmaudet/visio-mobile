import type { ScenarioContext } from "../../framework/participants/types.js";

export default async function(ctx: ScenarioContext) {
  const alice = ctx.bot("bot-a");
  const bob = ctx.bot("bot-b");
  const android = ctx.android();

  // Alice speaks
  await alice.speak();
  await alice.waitForEvent(/ActiveSpeakers.*bot-a/, 5000);
  await ctx.sleep(2000);

  const aliceSid = alice.sid;
  const bobSid = bob.sid;

  // Pin Alice
  ctx.log("Pinning Alice");
  await android.assertTestTag(`main-tile:${aliceSid}`, { timeout: 5000 });
  await android.longPress(`main-tile:${aliceSid}`);
  await ctx.sleep(500);
  await android.assertTestTag(`pin-indicator:${aliceSid}`, { timeout: 3000 });

  // Bob speaks — pin should hold
  ctx.log("Bob speaks — verifying pin holds");
  await alice.mute();
  await bob.speak();
  await bob.waitForEvent(/ActiveSpeakers.*bot-b/, 5000);
  await ctx.sleep(2000);

  // Main tile should STILL be Alice (pinned)
  await android.assertTestTag(`main-tile:${aliceSid}`, { timeout: 3000 });
  // Speaker border should be on Bob (in secondary tiles)
  await android.assertTestTag(`speaker-border:${bobSid}`, { timeout: 3000 });
  await android.screenshot("03-pin-holds-bob-speaking");

  await bob.mute();
  ctx.log("PASS: Pin holds despite speaker change");
}
