import type { ScenarioContext } from "../../framework/participants/types.js";

export default async function(ctx: ScenarioContext) {
  const alice = ctx.bot("bot-a");
  const bob = ctx.bot("bot-b");
  const android = ctx.android();
  const desktop = ctx.desktop();

  // Alice speaks first
  ctx.log("Alice speaks");
  await alice.speak();
  await alice.waitForEvent(/ActiveSpeakers.*bot-a/, 5000);
  await ctx.sleep(1500);

  const aliceSid = alice.sid;
  const bobSid = bob.sid;

  // Verify Alice in main tile
  await android.assertTestTag(`main-tile:${aliceSid}`, { timeout: 5000 });
  await android.screenshot("03-alice-initial-focus");

  // Bob speaks after only 1.5s — within stabilization window
  ctx.log("Bob speaks (within 2.5s stabilization window)");
  await alice.mute();
  await bob.speak();
  await bob.waitForEvent(/ActiveSpeakers.*bot-b/, 5000);
  await ctx.sleep(500);

  // Main tile should STILL be Alice (stabilization holds)
  ctx.log("Verifying stabilization: Alice still in main tile");
  await android.assertTestTag(`main-tile:${aliceSid}`, { timeout: 2000 });
  await android.screenshot("03-stabilization-holds-alice");

  // Wait for stabilization to expire (total > 2.5s since Alice was focused)
  await ctx.sleep(2500);

  // NOW Bob should be in main tile
  ctx.log("After stabilization expires: Bob should be in main tile");
  await android.assertTestTag(`main-tile:${bobSid}`, { timeout: 5000 });
  await android.screenshot("03-bob-after-stabilization");

  await desktop.assertTestId(`main-tile:${bobSid}`, { timeout: 5000 });
  await desktop.screenshot("03-bob-after-stabilization");

  await bob.mute();
  ctx.log("PASS: Stabilization held for 2.5s before switching");
}
