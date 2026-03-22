import type { ScenarioContext } from "../../framework/participants/types.js";

export default async function(ctx: ScenarioContext) {
  ctx.log("Scenario 02: 2 participants — local speaks → remote stays in main tile (no self-focus)");

  const alice = ctx.bot("bot-alice");
  const android = ctx.android();
  const desktop = ctx.desktop();
  const ios = ctx.ios();

  // Alice speaks first to establish remote focus
  await alice.connect();
  if (android) await android.connect();
  if (desktop) await desktop.connect();
  if (ios) await ios.connect();

  ctx.log("Alice speaks to establish initial remote focus");
  await alice.speak();
  await alice.waitForEvent(/ActiveSpeakers.*bot-alice/, 5000);
  await ctx.sleep(2000);

  const aliceSid = alice.sid;

  // Verify Alice is in main tile on Android
  if (android) await android.assertTestTag(`main-tile:${aliceSid}`, { timeout: 10000 });

  // Alice mutes — main tile must NOT switch to local participant
  ctx.log("Alice mutes — verifying main tile stays on Alice (no self-focus)");
  await alice.mute();
  await ctx.sleep(2000); // Less than the 5s silence-to-grid timeout

  // Main tile should still be Alice — local mic activity must not steal focus
  if (android) {
    await android.assertTestTag(`main-tile:${aliceSid}`, { timeout: 10000 });
    await android.screenshot("02-no-self-focus-android");
  }

  if (desktop) {
    await desktop.assertTestId(`main-tile:${aliceSid}`, { timeout: 10000 });
    await desktop.screenshot("02-no-self-focus-desktop");
  }

  if (ios) {
    await ios.screenshot("02-no-self-focus-ios");
  }

  ctx.log("PASS: Local speaker does not trigger self-focus; remote stays in main tile");
}
