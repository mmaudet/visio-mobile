import type { ScenarioContext } from "../../framework/participants/types.js";

export default async function(ctx: ScenarioContext) {
  ctx.log("Scenario 04: Rapid speaker changes — Alice/Bob/Charlie alternate < 2s each → no ping-pong");

  const alice = ctx.bot("bot-alice");
  const bob = ctx.bot("bot-bob");
  const charlie = ctx.bot("bot-charlie");
  const android = ctx.android();
  const desktop = ctx.desktop();
  const ios = ctx.ios();

  // Connect platforms first so they are ready before the bots speak
  await android.connect();
  await desktop.connect();
  await ios.connect();

  // Connect all bots
  await alice.connect();
  await bob.connect();
  await charlie.connect();

  const aliceSid = alice.sid;

  // Alice speaks first to establish a stable initial focus
  ctx.log("Alice speaks to establish initial stable focus");
  await alice.speak();
  await alice.waitForEvent(/ActiveSpeakers.*bot-alice/, 10000);
  await ctx.sleep(3000); // Full stabilization settle

  await android.assertTestTag(`main-tile:${aliceSid}`, { timeout: 10000 });
  await android.screenshot("04-alice-initial-android");
  await ios.screenshot("04-alice-initial-ios");

  // Rapid alternation — each speaks < 2s, well inside the 2.5s stabilization window
  ctx.log("Rapid alternation: Bob 1s → Charlie 1s → Bob 1s (all < 2s each)");
  await alice.mute();

  await bob.speak();
  await ctx.sleep(1000);
  await bob.mute();

  await charlie.speak();
  await ctx.sleep(1000);
  await charlie.mute();

  await bob.speak();
  await ctx.sleep(800);

  // Main tile should still be Alice — no ping-pong during rapid sub-2s changes
  ctx.log("Verifying: main tile stable on Alice despite rapid changes");
  await android.assertTestTag(`main-tile:${aliceSid}`, { timeout: 10000 });
  await android.screenshot("04-no-pingpong-android");

  await desktop.assertTestId(`main-tile:${aliceSid}`, { timeout: 10000 });
  await desktop.screenshot("04-no-pingpong-desktop");

  await ios.screenshot("04-no-pingpong-ios");

  await bob.mute();
  ctx.log("PASS: No ping-pong — focus remains stable during rapid sub-2s speaker changes");
}
