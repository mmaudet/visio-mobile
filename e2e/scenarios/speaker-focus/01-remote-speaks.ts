import type { ScenarioContext } from "../../framework/participants/types.js";

export default async function(ctx: ScenarioContext) {
  const alice = ctx.bot("bot-a");
  const android = ctx.android();
  const desktop = ctx.desktop();

  ctx.log("Alice starts speaking");
  await alice.speak();
  await alice.waitForEvent(/ActiveSpeakers.*bot-a/, 5000);
  await ctx.sleep(1000); // Let UI settle

  const aliceSid = alice.sid;
  ctx.log(`Verifying main tile shows Alice (${aliceSid})`);

  // Android assertions
  await android.assertTestTag(`layout-mode:FOCUS`, { timeout: 5000 });
  await android.assertTestTag(`main-tile:${aliceSid}`, { timeout: 5000 });
  await android.screenshot("01-alice-speaking-main-tile");

  // Desktop assertions
  await desktop.assertTestId(`layout-mode:FOCUS`, { timeout: 5000 });
  await desktop.assertTestId(`main-tile:${aliceSid}`, { timeout: 5000 });
  await desktop.screenshot("01-alice-speaking-main-tile");

  await alice.mute();
  ctx.log("PASS: Remote speaker correctly shown in main tile");
}
