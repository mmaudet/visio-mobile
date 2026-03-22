import type { ScenarioContext } from "../../framework/participants/types.js";

export default async function(ctx: ScenarioContext) {
  ctx.log("Scenario 06: Pedestrian mode — silence does NOT return to grid (last speaker stays in main tile)");

  const alice = ctx.bot("bot-alice");
  const android = ctx.android();
  const desktop = ctx.desktop();
  const ios = ctx.ios();

  await alice.connect();
  if (android) await android.connect();
  if (desktop) await desktop.connect();
  if (ios) await ios.connect();

  // This test assumes the device is in Pedestrian adaptive mode.
  // The adaptive mode should have been set before this scenario runs
  // (e.g. via the mode toggle in settings or a deep-link parameter).
  ctx.log("Note: device must be in Pedestrian adaptive mode for this scenario");

  // Alice speaks to enter FOCUS mode
  ctx.log("Alice speaks to establish FOCUS mode");
  await alice.speak();
  await alice.waitForEvent(/ActiveSpeakers.*bot-alice/, 5000);
  await ctx.sleep(2000);

  const aliceSid = alice.sid;

  if (android) {
    await android.assertTestTag(`main-tile:${aliceSid}`, { timeout: 10000 });
    await android.screenshot("06-focus-while-speaking-android");
  }

  if (ios) await ios.screenshot("06-focus-while-speaking-ios");

  // All mute — wait longer than the Office silence timeout
  ctx.log("Alice mutes — waiting 7s (well past 5s Office threshold)");
  await alice.mute();
  await ctx.sleep(7000);

  // In Pedestrian mode the last speaker should remain in main tile, NOT grid
  ctx.log("Verifying main tile still shows Alice (no grid return in Pedestrian mode)");
  if (android) {
    await android.assertTestTag(`main-tile:${aliceSid}`, { timeout: 10000 });
    await android.screenshot("06-pedestrian-still-focused-android");
  }

  if (desktop) {
    await desktop.assertTestId(`main-tile:${aliceSid}`, { timeout: 10000 });
    await desktop.screenshot("06-pedestrian-still-focused-desktop");
  }

  if (ios) await ios.screenshot("06-pedestrian-still-focused-ios");

  ctx.log("PASS: Pedestrian mode keeps last speaker in main tile after silence (no grid return)");
}
