// Layout Engine — unified speaker focus logic for Desktop
// Types are imported from App (they're already defined there)

export type LayoutMode = "grid" | "focus";

export interface LayoutDecision<T extends LayoutDisplayItem = LayoutDisplayItem> {
  mode: LayoutMode;
  mainTile: T | null;
  secondaryTiles: T[];
  speakerIndicatorSid: string | null;
  pinnedIndicatorSid: string | null;
}

export interface LayoutState {
  currentFocus: FocusItemNonNull | null;
  focusHoldStartMs: number | null;
  lastRemoteSpeakerSid: string | null;
}

// Non-nullable focus item for internal use
export interface FocusItemNonNull {
  participantSid: string;
  source: "camera" | "screen_share";
}

// Minimal shape for display items — compatible with App's DisplayItem
export interface LayoutDisplayItem {
  key: string;
  participant: { sid: string };
  source: "camera" | "screen_share";
  trackSid: string | null;
  label: string;
  isScreenShare: boolean;
}

const MIN_HOLD_MS = 2500;
const SILENCE_TO_GRID_MS = 5000;

export function computeLayout<T extends LayoutDisplayItem>(
  displayItems: T[],
  activeSpeakers: string[],
  pinnedItem: FocusItemNonNull | null,
  screenShare: FocusItemNonNull | null,
  localParticipantSid: string,
  previousState: LayoutState,
  nowMs: number,
): [LayoutDecision<T>, LayoutState] {
  // 1. Screen share has absolute priority
  if (screenShare) {
    const main = displayItems.find(
      d => d.participant.sid === screenShare.participantSid && d.source === screenShare.source
    ) ?? null;
    const secondary = displayItems.filter(d => d.key !== main?.key);
    const speakerSid = activeSpeakers[0] ?? null;
    return [
      { mode: "focus", mainTile: main, secondaryTiles: secondary, speakerIndicatorSid: speakerSid, pinnedIndicatorSid: pinnedItem?.participantSid ?? null },
      { ...previousState, currentFocus: screenShare },
    ];
  }

  // 2. Pin has priority over auto-focus
  if (pinnedItem) {
    const main = displayItems.find(
      d => d.participant.sid === pinnedItem.participantSid && d.source === pinnedItem.source
    ) ?? null;
    const secondary = displayItems.filter(d => d.key !== main?.key);
    const speakerSid = activeSpeakers[0] ?? null;
    return [
      { mode: "focus", mainTile: main, secondaryTiles: secondary, speakerIndicatorSid: speakerSid, pinnedIndicatorSid: pinnedItem.participantSid },
      { ...previousState, currentFocus: pinnedItem },
    ];
  }

  // 3. Active speaker logic with stabilization
  const currentSpeakerSid = activeSpeakers[0] ?? null;
  const isLocalSpeaking = currentSpeakerSid === localParticipantSid;

  if (currentSpeakerSid) {
    const newLastRemote = !isLocalSpeaking ? currentSpeakerSid : previousState.lastRemoteSpeakerSid;

    const targetSid = isLocalSpeaking
      ? (previousState.lastRemoteSpeakerSid ?? null)
      : currentSpeakerSid;

    if (targetSid) {
      const targetFocus: FocusItemNonNull = { participantSid: targetSid, source: "camera" };

      let shouldSwitch: boolean;
      if (!previousState.currentFocus) {
        shouldSwitch = true;
      } else if (
        previousState.currentFocus.participantSid === targetFocus.participantSid &&
        previousState.currentFocus.source === targetFocus.source
      ) {
        shouldSwitch = false;
      } else {
        const holdElapsed = previousState.focusHoldStartMs != null
          ? nowMs - previousState.focusHoldStartMs
          : Infinity;
        shouldSwitch = holdElapsed >= MIN_HOLD_MS;
      }

      if (shouldSwitch) {
        const main = displayItems.find(d => d.participant.sid === targetSid && d.source === "camera") ?? null;
        const secondary = displayItems.filter(d => d.key !== main?.key);
        return [
          { mode: "focus", mainTile: main, secondaryTiles: secondary, speakerIndicatorSid: currentSpeakerSid, pinnedIndicatorSid: null },
          { currentFocus: targetFocus, focusHoldStartMs: nowMs, lastRemoteSpeakerSid: newLastRemote },
        ];
      } else {
        const currentMain = displayItems.find(
          d => d.participant.sid === previousState.currentFocus?.participantSid &&
               d.source === previousState.currentFocus?.source
        ) ?? null;
        const secondary = displayItems.filter(d => d.key !== currentMain?.key);
        return [
          { mode: "focus", mainTile: currentMain, secondaryTiles: secondary, speakerIndicatorSid: currentSpeakerSid, pinnedIndicatorSid: null },
          { ...previousState, lastRemoteSpeakerSid: newLastRemote },
        ];
      }
    }
  }

  // 4. No speaker — check silence timeout (Desktop is always "office" mode)
  const silenceElapsed = previousState.focusHoldStartMs != null
    ? nowMs - previousState.focusHoldStartMs
    : Infinity;
  if (silenceElapsed > SILENCE_TO_GRID_MS) {
    return [
      { mode: "grid", mainTile: null, secondaryTiles: displayItems, speakerIndicatorSid: null, pinnedIndicatorSid: null },
      { currentFocus: null, focusHoldStartMs: null, lastRemoteSpeakerSid: previousState.lastRemoteSpeakerSid },
    ];
  }

  // Keep current state
  if (previousState.currentFocus) {
    const currentMain = displayItems.find(
      d => d.participant.sid === previousState.currentFocus!.participantSid &&
           d.source === previousState.currentFocus!.source
    ) ?? null;
    if (currentMain) {
      const secondary = displayItems.filter(d => d.key !== currentMain.key);
      return [
        { mode: "focus", mainTile: currentMain, secondaryTiles: secondary, speakerIndicatorSid: null, pinnedIndicatorSid: null },
        previousState,
      ];
    }
  }

  // Default: grid
  return [
    { mode: "grid", mainTile: null, secondaryTiles: displayItems, speakerIndicatorSid: null, pinnedIndicatorSid: null },
    { ...previousState, currentFocus: null },
  ];
}

export function initialLayoutState(): LayoutState {
  return { currentFocus: null, focusHoldStartMs: null, lastRemoteSpeakerSid: null };
}
