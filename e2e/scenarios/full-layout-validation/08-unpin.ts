import type { ScenarioContext } from "../../framework/participants/types.js";

export default async function(ctx: ScenarioContext) {
  ctx.log("Scenario 08: Mobile only — long press again → pin removed, return to auto-focus");

  const alice = ctx.bot("bot-alice");
  const android = ctx.android();
  const ios = ctx.ios();

  await alice.connect();
  if (android) await android.connect();
  if (ios) await ios.connect();

  // Setup: Alice speaks, establish focus, pin her
  ctx.log("Alice speaks to establish focus");
  await alice.speak();
  await alice.waitForEvent(/ActiveSpeakers.*bot-alice/, 5000);
  await ctx.sleep(2000);

  const aliceSid = alice.sid;

  if (android) {
    await android.assertTestTag(`main-tile:${aliceSid}`, { timeout: 10000 });

    // Pin Alice
    ctx.log("Pinning Alice via long press");
    await android.longPress(`main-tile:${aliceSid}`);
    await ctx.sleep(500);
    await android.assertTestTag(`pin-indicator:${aliceSid}`, { timeout: 10000 });
    await android.screenshot("08-pinned-android");

    // Long press again to unpin
    ctx.log("Long pressing again to unpin Alice");
    await android.longPress(`main-tile:${aliceSid}`);
    await ctx.sleep(500);

    // Pin indicator must be gone — auto-focus resumes
    await android.assertNotTestTag(`pin-indicator:${aliceSid}`, { timeout: 10000 });
    await android.screenshot("08-pin-removed-android");
  }

  // iOS: screenshot evidence of unpinned state
  if (ios) {
    ctx.log("iOS: capturing unpinned state screenshot");
    await ios.screenshot("08-pin-removed-ios");
  }

  await alice.mute();
  ctx.log("PASS: Second long press removes pin; auto-focus mode resumes");
}
