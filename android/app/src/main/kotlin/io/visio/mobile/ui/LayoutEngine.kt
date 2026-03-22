package io.visio.mobile.ui

import uniffi.visio.AdaptiveMode
import uniffi.visio.ParticipantInfo

// NOTE: FocusItem and DisplayItem are already defined in CallScreen.kt, reuse them
// NOTE: buildDisplayItems is already defined in CallScreen.kt, reuse it

enum class LayoutMode { GRID, FOCUS }

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

private const val MIN_HOLD_MS = 2500L
private const val SILENCE_TO_GRID_MS = 5000L

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

    val speakerResult =
        computeSpeakerLayout(
            displayItems,
            activeSpeakers,
            localParticipantSid,
            participants,
            previousState,
            nowMs,
        )
    if (speakerResult != null) return speakerResult

    return computeFallbackLayout(displayItems, previousState, nowMs, adaptiveMode)
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

private fun computeSpeakerLayout(
    displayItems: List<DisplayItem>,
    activeSpeakers: List<String>,
    localParticipantSid: String,
    participants: List<ParticipantInfo>,
    previousState: LayoutState,
    nowMs: Long,
): Pair<LayoutDecision, LayoutState>? {
    val currentSpeakerSid = activeSpeakers.firstOrNull() ?: return null
    val isLocalSpeaking = currentSpeakerSid == localParticipantSid
    val newLastRemote = if (!isLocalSpeaking) currentSpeakerSid else previousState.lastRemoteSpeakerSid

    val targetSid =
        resolveTargetSpeaker(isLocalSpeaking, currentSpeakerSid, previousState, participants)
            ?: return null

    val targetFocus = FocusItem(targetSid, "camera")
    val shouldSwitch = shouldSwitchFocus(previousState, targetFocus, nowMs)

    return if (shouldSwitch) {
        val main = findDisplayItem(displayItems, targetSid, "camera")
        val secondary = displayItems.filter { it.key != main?.key }
        Pair(
            LayoutDecision(LayoutMode.FOCUS, main, secondary, currentSpeakerSid, null),
            LayoutState(targetFocus, nowMs, newLastRemote),
        )
    } else {
        val currentMain =
            previousState.currentFocus?.let {
                findDisplayItem(displayItems, it.participantSid, it.source)
            }
        val secondary = displayItems.filter { it.key != currentMain?.key }
        Pair(
            LayoutDecision(LayoutMode.FOCUS, currentMain, secondary, currentSpeakerSid, null),
            previousState.copy(lastRemoteSpeakerSid = newLastRemote),
        )
    }
}

private fun resolveTargetSpeaker(
    isLocalSpeaking: Boolean,
    currentSpeakerSid: String,
    previousState: LayoutState,
    participants: List<ParticipantInfo>,
): String? =
    if (isLocalSpeaking) {
        previousState.lastRemoteSpeakerSid ?: participants.drop(1).firstOrNull()?.sid
    } else {
        currentSpeakerSid
    }

private fun shouldSwitchFocus(
    previousState: LayoutState,
    targetFocus: FocusItem,
    nowMs: Long,
): Boolean =
    when {
        previousState.currentFocus == null -> true
        previousState.currentFocus == targetFocus -> false
        else -> {
            val holdElapsed = previousState.focusHoldStartMs?.let { nowMs - it } ?: Long.MAX_VALUE
            holdElapsed >= MIN_HOLD_MS
        }
    }

private fun computeFallbackLayout(
    displayItems: List<DisplayItem>,
    previousState: LayoutState,
    nowMs: Long,
    adaptiveMode: AdaptiveMode,
): Pair<LayoutDecision, LayoutState> {
    val silenceElapsed = previousState.focusHoldStartMs?.let { nowMs - it } ?: Long.MAX_VALUE
    if (silenceElapsed > SILENCE_TO_GRID_MS && adaptiveMode == AdaptiveMode.OFFICE) {
        return Pair(
            LayoutDecision(LayoutMode.GRID, null, displayItems, null, null),
            previousState.copy(currentFocus = null, focusHoldStartMs = null),
        )
    }

    val currentMain =
        previousState.currentFocus?.let { focus ->
            findDisplayItem(displayItems, focus.participantSid, focus.source)
        }
    if (currentMain != null) {
        val secondary = displayItems.filter { it.key != currentMain.key }
        return Pair(
            LayoutDecision(LayoutMode.FOCUS, currentMain, secondary, null, null),
            previousState,
        )
    }

    return Pair(
        LayoutDecision(LayoutMode.GRID, null, displayItems, null, null),
        previousState.copy(currentFocus = null),
    )
}

private fun findDisplayItem(
    displayItems: List<DisplayItem>,
    participantSid: String,
    source: String,
): DisplayItem? = displayItems.find { it.participant.sid == participantSid && it.source == source }
