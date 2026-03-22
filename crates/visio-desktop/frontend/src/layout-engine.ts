// Layout Engine — unified speaker focus logic for Desktop
// Types are imported from App (they're already defined there)

export type LayoutMode = 'grid' | 'focus'

export interface LayoutDecision<
  T extends LayoutDisplayItem = LayoutDisplayItem,
> {
  mode: LayoutMode
  mainTile: T | null
  secondaryTiles: T[]
  speakerIndicatorSid: string | null
  pinnedIndicatorSid: string | null
}

export interface LayoutState {
  currentFocus: FocusItemNonNull | null
  focusHoldStartMs: number | null
  lastRemoteSpeakerSid: string | null
}

// Non-nullable focus item for internal use
export interface FocusItemNonNull {
  participantSid: string
  source: 'camera' | 'screen_share'
}

// Minimal shape for display items — compatible with App's DisplayItem
export interface LayoutDisplayItem {
  key: string
  participant: { sid: string }
  source: 'camera' | 'screen_share'
  trackSid: string | null
  label: string
  isScreenShare: boolean
}

const MIN_HOLD_MS = 2500
const SILENCE_TO_GRID_MS = 5000

/** Find a display item matching a focus target (participant + source). */
function findByFocus<T extends LayoutDisplayItem>(
  items: T[],
  focus: FocusItemNonNull
): T | null {
  return (
    items.find(
      (d) =>
        d.participant.sid === focus.participantSid && d.source === focus.source
    ) ?? null
  )
}

/** Build a focus-mode layout result with a main tile and remaining secondaries. */
function buildFocusResult<T extends LayoutDisplayItem>(
  items: T[],
  main: T | null,
  speakerSid: string | null,
  pinnedSid: string | null,
  state: LayoutState
): [LayoutDecision<T>, LayoutState] {
  const secondary = items.filter((d) => d.key !== main?.key)
  return [
    {
      mode: 'focus',
      mainTile: main,
      secondaryTiles: secondary,
      speakerIndicatorSid: speakerSid,
      pinnedIndicatorSid: pinnedSid,
    },
    state,
  ]
}

/** Build a grid-mode layout result (no main tile). */
function buildGridResult<T extends LayoutDisplayItem>(
  items: T[],
  state: LayoutState
): [LayoutDecision<T>, LayoutState] {
  return [
    {
      mode: 'grid',
      mainTile: null,
      secondaryTiles: items,
      speakerIndicatorSid: null,
      pinnedIndicatorSid: null,
    },
    state,
  ]
}

/** Determine whether focus should switch to a new target. */
function shouldSwitchFocus(
  previousState: LayoutState,
  targetFocus: FocusItemNonNull,
  nowMs: number
): boolean {
  if (previousState.currentFocus === null) {
    return true
  }
  if (
    previousState.currentFocus.participantSid === targetFocus.participantSid &&
    previousState.currentFocus.source === targetFocus.source
  ) {
    return false
  }
  const holdElapsed =
    previousState.focusHoldStartMs != null
      ? nowMs - previousState.focusHoldStartMs
      : Infinity
  return holdElapsed >= MIN_HOLD_MS
}

/** Handle active speaker logic with stabilization (step 3). */
function handleActiveSpeaker<T extends LayoutDisplayItem>(
  displayItems: T[],
  currentSpeakerSid: string,
  localParticipantSid: string,
  previousState: LayoutState,
  nowMs: number
): [LayoutDecision<T>, LayoutState] | null {
  const isLocalSpeaking = currentSpeakerSid === localParticipantSid

  const newLastRemote = isLocalSpeaking
    ? previousState.lastRemoteSpeakerSid
    : currentSpeakerSid

  const targetSid = isLocalSpeaking
    ? (previousState.lastRemoteSpeakerSid ?? null)
    : currentSpeakerSid

  if (targetSid === null) {
    return null
  }

  const targetFocus: FocusItemNonNull = {
    participantSid: targetSid,
    source: 'camera',
  }

  if (shouldSwitchFocus(previousState, targetFocus, nowMs)) {
    const main = findByFocus(displayItems, targetFocus)
    return buildFocusResult(displayItems, main, currentSpeakerSid, null, {
      currentFocus: targetFocus,
      focusHoldStartMs: nowMs,
      lastRemoteSpeakerSid: newLastRemote,
    })
  }

  const currentMain = previousState.currentFocus
    ? findByFocus(displayItems, previousState.currentFocus)
    : null
  return buildFocusResult(displayItems, currentMain, currentSpeakerSid, null, {
    ...previousState,
    lastRemoteSpeakerSid: newLastRemote,
  })
}

export function computeLayout<T extends LayoutDisplayItem>(
  displayItems: T[],
  activeSpeakers: string[],
  pinnedItem: FocusItemNonNull | null,
  screenShare: FocusItemNonNull | null,
  localParticipantSid: string,
  previousState: LayoutState,
  nowMs: number
): [LayoutDecision<T>, LayoutState] {
  // 1. Screen share has absolute priority
  if (screenShare) {
    const main = findByFocus(displayItems, screenShare)
    const speakerSid = activeSpeakers[0] ?? null
    return buildFocusResult(
      displayItems,
      main,
      speakerSid,
      pinnedItem?.participantSid ?? null,
      { ...previousState, currentFocus: screenShare }
    )
  }

  // 2. Pin has priority over auto-focus
  if (pinnedItem) {
    const main = findByFocus(displayItems, pinnedItem)
    const speakerSid = activeSpeakers[0] ?? null
    return buildFocusResult(
      displayItems,
      main,
      speakerSid,
      pinnedItem.participantSid,
      { ...previousState, currentFocus: pinnedItem }
    )
  }

  // 3. Active speaker logic with stabilization
  const currentSpeakerSid = activeSpeakers[0] ?? null
  if (currentSpeakerSid) {
    const result = handleActiveSpeaker(
      displayItems,
      currentSpeakerSid,
      localParticipantSid,
      previousState,
      nowMs
    )
    if (result) return result
  }

  // 4. No speaker — check silence timeout (Desktop is always "office" mode)
  const silenceElapsed =
    previousState.focusHoldStartMs != null
      ? nowMs - previousState.focusHoldStartMs
      : Infinity
  if (silenceElapsed > SILENCE_TO_GRID_MS) {
    return buildGridResult(displayItems, {
      currentFocus: null,
      focusHoldStartMs: null,
      lastRemoteSpeakerSid: previousState.lastRemoteSpeakerSid,
    })
  }

  // Keep current state
  if (previousState.currentFocus) {
    const currentMain = findByFocus(displayItems, previousState.currentFocus)
    if (currentMain) {
      return buildFocusResult(
        displayItems,
        currentMain,
        null,
        null,
        previousState
      )
    }
  }

  // Default: grid
  return buildGridResult(displayItems, {
    ...previousState,
    currentFocus: null,
  })
}

export function initialLayoutState(): LayoutState {
  return {
    currentFocus: null,
    focusHoldStartMs: null,
    lastRemoteSpeakerSid: null,
  }
}
