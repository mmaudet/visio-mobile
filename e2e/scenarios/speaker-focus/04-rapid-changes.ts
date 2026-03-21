import type { ScenarioContext } from "../../framework/participants/types.js";

export default async function(ctx: ScenarioContext) {
  const alice = ctx.bot("bot-a");
  const bob = ctx.bot("bot-b");
  const charlie = ctx.bot("bot-c");
  const android = ctx.android();

  // Alice speaks first to establish initial focus
  ctx.log("Alice speaks to establish initial focus");
  await alice.speak();
  await alice.waitForEvent(/ActiveSpeakers.*bot-a/, 5000);
  await ctx.sleep(3000); // Let stabilization settle

  const aliceSid = alice.sid;
  await android.assertTestTag(`main-tile:${aliceSid}`, { timeout: 5000 });
  await android.screenshot("04-alice-initial");

  // Rapid alternation: each speaks < 2s
  ctx.log("Rapid alternation: Bob 1s → Charlie 1s → Bob 1s");
  await alice.mute();
  await bob.speak();
  await ctx.sleep(1000);
  await bob.mute();
  await charlie.speak();
  await ctx.sleep(1000);
  await charlie.mute();
  await bob.speak();
  await ctx.sleep(500);

  // Main tile should STILL be Alice (no ping-pong during rapid changes)
  ctx.log("Verifying: main tile stable during rapid changes");
  await android.assertTestTag(`main-tile:${aliceSid}`, { timeout: 2000 });
  await android.screenshot("04-no-pingpong");

  await bob.mute();
  ctx.log("PASS: No ping-pong during rapid speaker changes");
}
