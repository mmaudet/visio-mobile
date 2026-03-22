import type { ScenarioContext } from "../../framework/participants/types.js";

export default async function(ctx: ScenarioContext) {
  ctx.log("Scenario 09: Pin Alice → Bob speaks → pin holds, blue border on Bob's thumbnail");

  const alice = ctx.bot("bot-alice");
  const bob = ctx.bot("bot-bob");
  const android = ctx.android();
  const desktop = ctx.desktop();
  const ios = ctx.ios();

  await alice.connect();
  await bob.connect();
  if (android) await android.connect();
  if (desktop) await desktop.connect();
  if (ios) await ios.connect();

  const aliceSid = alice.sid;
  const bobSid = bob.sid;

  // Alice speaks to establish focus
  ctx.log("Alice speaks to establish focus");
  await alice.speak();
  await alice.waitForEvent(/ActiveSpeakers.*bot-alice/, 5000);
  await ctx.sleep(2000);

  if (android) {
    await android.assertTestTag(`main-tile:${aliceSid}`, { timeout: 5000 });

    // Pin Alice on Android
    ctx.log("Pinning Alice via long press");
    await android.longPress(`main-tile:${aliceSid}`);
    await ctx.sleep(500);
    await android.assertTestTag(`pin-indicator:${aliceSid}`, { timeout: 3000 });
    await android.screenshot("09-alice-pinned-android");
  }

  // Bob speaks — pin must hold (Alice stays in main tile)
  ctx.log("Bob speaks (French TTS) — pin must hold on Alice");
  await alice.mute();
  await bob.speak();
  await bob.waitForEvent(/ActiveSpeakers.*bot-bob/, 5000);
  await ctx.sleep(2000);

  if (android) {
    // Main tile must still be Alice (pinned) despite Bob speaking
    await android.assertTestTag(`main-tile:${aliceSid}`, { timeout: 3000 });

    // Bob's thumbnail must show the speaker border (active speaker indicator)
    await android.assertTestTag(`speaker-border:${bobSid}`, { timeout: 3000 });
    await android.screenshot("09-pin-holds-bob-border-android");
  }

  // Desktop: pin is a mobile-only gesture, but desktop still shows auto-focus on Bob
  // (no pin concept on desktop — Bob should be in main tile there)
  if (desktop) {
    await desktop.assertTestId(`main-tile:${bobSid}`, { timeout: 5000 });
    await desktop.screenshot("09-bob-in-desktop-main-tile");
  }

  // iOS: screenshot evidence
  if (ios) await ios.screenshot("09-pin-holds-ios");

  await bob.mute();
  ctx.log("PASS: Pin holds on Android despite speaker change; Bob shows speaker border");
}
