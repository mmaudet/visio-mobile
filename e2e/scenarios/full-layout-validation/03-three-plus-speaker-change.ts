import type { ScenarioContext } from "../../framework/participants/types.js";

export default async function(ctx: ScenarioContext) {
  ctx.log("Scenario 03: 3+ participants — Alice speaks 5s → Bob speaks → focus switches after 2.5s stabilization");

  const alice = ctx.bot("bot-alice");
  const bob = ctx.bot("bot-bob");
  const android = ctx.android();
  const desktop = ctx.desktop();
  const ios = ctx.ios();

  // Connect both bots
  await alice.connect();
  await bob.connect();

  const aliceSid = alice.sid;
  const bobSid = bob.sid;

  // Alice speaks to establish focus
  ctx.log("Alice speaks (French TTS) — establishing focus");
  await alice.speak();
  await alice.waitForEvent(/ActiveSpeakers.*bot-alice/, 5000);
  await ctx.sleep(5000); // Alice speaks 5 full seconds

  await android.assertTestTag("layout-mode:FOCUS", { timeout: 5000 });
  await android.assertTestTag(`main-tile:${aliceSid}`, { timeout: 5000 });
  await android.screenshot("03-alice-initial-focus-android");
  await ios.screenshot("03-alice-initial-focus-ios");

  // Alice mutes, Bob starts speaking — stabilization should hold for ~2.5s
  ctx.log("Alice mutes, Bob speaks — stabilization window holds");
  await alice.mute();
  await bob.speak();
  await bob.waitForEvent(/ActiveSpeakers.*bot-bob/, 5000);
  await ctx.sleep(500); // Well within stabilization window

  // Main tile should still be Alice (stabilization in effect)
  await android.assertTestTag(`main-tile:${aliceSid}`, { timeout: 2000 });
  await android.screenshot("03-stabilization-holds-android");

  // Wait for stabilization window to expire
  ctx.log("Waiting for 2.5s stabilization to expire");
  await ctx.sleep(2500);

  // Bob should now be in main tile
  ctx.log("After stabilization: Bob should be in main tile");
  await android.assertTestTag(`main-tile:${bobSid}`, { timeout: 5000 });
  await android.screenshot("03-bob-after-stabilization-android");

  await desktop.assertTestId(`main-tile:${bobSid}`, { timeout: 5000 });
  await desktop.screenshot("03-bob-after-stabilization-desktop");

  await ios.screenshot("03-bob-after-stabilization-ios");

  await bob.mute();
  ctx.log("PASS: Focus switched to Bob after 2.5s stabilization window expired");
}
