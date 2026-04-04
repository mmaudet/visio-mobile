package io.visio.mobile.ui

import uniffi.visio.AdaptiveMode
import uniffi.visio.ParticipantInfo

// NOTE: FocusItem and DisplayItem are already defined in CallScreen.kt, reuse them
// NOTE: buildDisplayItems is already defined in CallScreen.kt, reuse it

enum class LayoutMode { GRID, FOCUS, SPEAKER }

data class LayoutDecision(
    val mode: LayoutMode,
    val mainTile: DisplayItem?,
    val secondaryTiles: List<DisplayItem>,
    val speakerIndicatorSid: String?,
    val pinnedIndicatorSid: String?,
)

data class LayoutState(
    val currentFocus: FocusItem? = null,
    val focusHoldStartMs: Long? = null,
    val lastRemoteSpeakerSid: String? = null,
)

@Suppress("kotlin:S107")
fun computeLayout(
    participants: List<ParticipantInfo>,
    activeSpeakers: List<String>,
    pinnedItem: FocusItem?,
    screenShare: FocusItem?,
    adaptiveMode: AdaptiveMode,
    localParticipantSid: String,
    previousState: LayoutState,
    nowMs: Long,
): Pair<LayoutDecision, LayoutState> {
    val displayItems = buildDisplayItems(participants)

    screenShare?.let {
        return computeScreenShareLayout(displayItems, it, activeSpeakers, pinnedItem, previousState)
    }
    pinnedItem?.let {
        return computePinnedLayout(displayItems, it, activeSpeakers, previousState)
    }

    // Default: grid layout (no auto-focus on active speaker)
    return Pair(
        LayoutDecision(LayoutMode.GRID, null, displayItems, activeSpeakers.firstOrNull(), null),
        previousState.copy(currentFocus = null, focusHoldStartMs = null),
    )
}

private fun computeScreenShareLayout(
    displayItems: List<DisplayItem>,
    screenShare: FocusItem,
    activeSpeakers: List<String>,
    pinnedItem: FocusItem?,
    previousState: LayoutState,
): Pair<LayoutDecision, LayoutState> {
    val main = findDisplayItem(displayItems, screenShare.participantSid, screenShare.source)
    val secondary = displayItems.filter { it.key != main?.key }
    return Pair(
        LayoutDecision(LayoutMode.FOCUS, main, secondary, activeSpeakers.firstOrNull(), pinnedItem?.participantSid),
        previousState.copy(currentFocus = screenShare),
    )
}

private fun computePinnedLayout(
    displayItems: List<DisplayItem>,
    pinnedItem: FocusItem,
    activeSpeakers: List<String>,
    previousState: LayoutState,
): Pair<LayoutDecision, LayoutState> {
    val main = findDisplayItem(displayItems, pinnedItem.participantSid, pinnedItem.source)
    val secondary = displayItems.filter { it.key != main?.key }
    return Pair(
        LayoutDecision(LayoutMode.FOCUS, main, secondary, activeSpeakers.firstOrNull(), pinnedItem.participantSid),
        previousState.copy(currentFocus = pinnedItem),
    )
}

private fun findDisplayItem(
    displayItems: List<DisplayItem>,
    participantSid: String,
    source: String,
): DisplayItem? = displayItems.find { it.participant.sid == participantSid && it.source == source }
