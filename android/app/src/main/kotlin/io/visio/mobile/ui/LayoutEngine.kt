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

    // 1. Screen share has absolute priority
    if (screenShare != null) {
        val main =
            displayItems.find {
                it.participant.sid == screenShare.participantSid && it.source == screenShare.source
            }
        val secondary = displayItems.filter { it.key != main?.key }
        val speakerSid = activeSpeakers.firstOrNull()
        return Pair(
            LayoutDecision(LayoutMode.FOCUS, main, secondary, speakerSid, pinnedItem?.participantSid),
            previousState.copy(currentFocus = screenShare),
        )
    }

    // 2. Pin has priority over auto-focus
    if (pinnedItem != null) {
        val main =
            displayItems.find {
                it.participant.sid == pinnedItem.participantSid && it.source == pinnedItem.source
            }
        val secondary = displayItems.filter { it.key != main?.key }
        val speakerSid = activeSpeakers.firstOrNull()
        return Pair(
            LayoutDecision(LayoutMode.FOCUS, main, secondary, speakerSid, pinnedItem.participantSid),
            previousState.copy(currentFocus = pinnedItem),
        )
    }

    // 3. Active speaker logic with stabilization
    val currentSpeakerSid = activeSpeakers.firstOrNull()
    val isLocalSpeaking = currentSpeakerSid == localParticipantSid

    if (currentSpeakerSid != null) {
        val newLastRemote = if (!isLocalSpeaking) currentSpeakerSid else previousState.lastRemoteSpeakerSid

        val targetSid =
            if (isLocalSpeaking) {
                previousState.lastRemoteSpeakerSid ?: participants.drop(1).firstOrNull()?.sid
            } else {
                currentSpeakerSid
            }

        if (targetSid != null) {
            val targetFocus = FocusItem(targetSid, "camera")

            val shouldSwitch =
                if (previousState.currentFocus == null) {
                    true
                } else if (previousState.currentFocus == targetFocus) {
                    false
                } else {
                    val holdElapsed = previousState.focusHoldStartMs?.let { nowMs - it } ?: Long.MAX_VALUE
                    holdElapsed >= MIN_HOLD_MS
                }

            if (shouldSwitch) {
                val main = displayItems.find { it.participant.sid == targetSid && it.source == "camera" }
                val secondary = displayItems.filter { it.key != main?.key }
                return Pair(
                    LayoutDecision(LayoutMode.FOCUS, main, secondary, currentSpeakerSid, null),
                    LayoutState(targetFocus, nowMs, newLastRemote),
                )
            } else {
                val currentMain =
                    displayItems.find {
                        it.participant.sid == previousState.currentFocus?.participantSid &&
                            it.source == previousState.currentFocus?.source
                    }
                val secondary = displayItems.filter { it.key != currentMain?.key }
                return Pair(
                    LayoutDecision(LayoutMode.FOCUS, currentMain, secondary, currentSpeakerSid, null),
                    previousState.copy(lastRemoteSpeakerSid = newLastRemote),
                )
            }
        }
    }

    // 4. No speaker — check silence timeout
    val silenceElapsed = previousState.focusHoldStartMs?.let { nowMs - it } ?: Long.MAX_VALUE
    if (silenceElapsed > SILENCE_TO_GRID_MS && adaptiveMode == AdaptiveMode.OFFICE) {
        return Pair(
            LayoutDecision(LayoutMode.GRID, null, displayItems, null, null),
            previousState.copy(currentFocus = null, focusHoldStartMs = null),
        )
    }

    // Keep current state
    val currentMain =
        previousState.currentFocus?.let { focus ->
            displayItems.find { it.participant.sid == focus.participantSid && it.source == focus.source }
        }
    if (currentMain != null) {
        val secondary = displayItems.filter { it.key != currentMain.key }
        return Pair(
            LayoutDecision(LayoutMode.FOCUS, currentMain, secondary, null, null),
            previousState,
        )
    }

    // Default: grid
    return Pair(
        LayoutDecision(LayoutMode.GRID, null, displayItems, null, null),
        previousState.copy(currentFocus = null),
    )
}
