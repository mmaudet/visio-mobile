import Foundation
import visioFFI

// MARK: - Layout Engine Types

enum LayoutMode {
    case grid
    case focus
}

struct LayoutDecision {
    let mode: LayoutMode
    let mainTile: DisplayItem?
    let secondaryTiles: [DisplayItem]
    let speakerIndicatorSid: String?
    let pinnedIndicatorSid: String?
}

struct LayoutState: Equatable {
    var currentFocus: FocusItem? = nil
    var focusHoldStartMs: Double? = nil
    var lastRemoteSpeakerSid: String? = nil
}

// MARK: - Layout Engine

private let minHoldMs: Double = 2500  // 2.5 seconds
private let silenceToGridMs: Double = 5000  // 5 seconds

func computeLayout(
    participants: [ParticipantInfo],
    activeSpeakers: [String],
    pinnedItem: FocusItem?,
    screenShare: FocusItem?,
    adaptiveMode: AdaptiveMode,
    localParticipantSid: String,
    previousState: LayoutState,
    nowMs: Double
) -> (LayoutDecision, LayoutState) {
    let displayItems = buildDisplayItems(participants)

    // 1. Screen share has absolute priority
    if let screenShare = screenShare {
        let main = displayItems.first {
            $0.participant.sid == screenShare.participantSid && $0.source == screenShare.source
        }
        let secondary = displayItems.filter { $0.id != main?.id }
        let speakerSid = activeSpeakers.first
        return (
            LayoutDecision(mode: .focus, mainTile: main, secondaryTiles: secondary, speakerIndicatorSid: speakerSid, pinnedIndicatorSid: pinnedItem?.participantSid),
            LayoutState(currentFocus: screenShare, focusHoldStartMs: previousState.focusHoldStartMs, lastRemoteSpeakerSid: previousState.lastRemoteSpeakerSid)
        )
    }

    // 2. Pin has priority over auto-focus
    if let pinnedItem = pinnedItem {
        let main = displayItems.first {
            $0.participant.sid == pinnedItem.participantSid && $0.source == pinnedItem.source
        }
        let secondary = displayItems.filter { $0.id != main?.id }
        let speakerSid = activeSpeakers.first
        return (
            LayoutDecision(mode: .focus, mainTile: main, secondaryTiles: secondary, speakerIndicatorSid: speakerSid, pinnedIndicatorSid: pinnedItem.participantSid),
            LayoutState(currentFocus: pinnedItem, focusHoldStartMs: previousState.focusHoldStartMs, lastRemoteSpeakerSid: previousState.lastRemoteSpeakerSid)
        )
    }

    // 3. Active speaker logic with stabilization
    let currentSpeakerSid = activeSpeakers.first
    let isLocalSpeaking = currentSpeakerSid == localParticipantSid

    if let currentSpeakerSid = currentSpeakerSid {
        let newLastRemote = !isLocalSpeaking ? currentSpeakerSid : previousState.lastRemoteSpeakerSid

        let targetSid: String? = isLocalSpeaking
            ? (previousState.lastRemoteSpeakerSid ?? participants.dropFirst().first?.sid)
            : currentSpeakerSid

        if let targetSid = targetSid {
            let targetFocus = FocusItem(participantSid: targetSid, source: .camera)

            let shouldSwitch: Bool
            if previousState.currentFocus == nil {
                shouldSwitch = true
            } else if previousState.currentFocus == targetFocus {
                shouldSwitch = false
            } else {
                let holdElapsed = previousState.focusHoldStartMs.map { nowMs - $0 } ?? Double.greatestFiniteMagnitude
                shouldSwitch = holdElapsed >= minHoldMs
            }

            if shouldSwitch {
                let main = displayItems.first { $0.participant.sid == targetSid && $0.source == .camera }
                let secondary = displayItems.filter { $0.id != main?.id }
                return (
                    LayoutDecision(mode: .focus, mainTile: main, secondaryTiles: secondary, speakerIndicatorSid: currentSpeakerSid, pinnedIndicatorSid: nil),
                    LayoutState(currentFocus: targetFocus, focusHoldStartMs: nowMs, lastRemoteSpeakerSid: newLastRemote)
                )
            } else {
                let currentMain = displayItems.first {
                    $0.participant.sid == previousState.currentFocus?.participantSid && $0.source == previousState.currentFocus?.source
                }
                let secondary = displayItems.filter { $0.id != currentMain?.id }
                return (
                    LayoutDecision(mode: .focus, mainTile: currentMain, secondaryTiles: secondary, speakerIndicatorSid: currentSpeakerSid, pinnedIndicatorSid: nil),
                    LayoutState(currentFocus: previousState.currentFocus, focusHoldStartMs: previousState.focusHoldStartMs, lastRemoteSpeakerSid: newLastRemote)
                )
            }
        }
    }

    // 4. No speaker — check silence timeout
    let silenceElapsed = previousState.focusHoldStartMs.map { nowMs - $0 } ?? Double.greatestFiniteMagnitude
    if silenceElapsed > silenceToGridMs && adaptiveMode == .office {
        return (
            LayoutDecision(mode: .grid, mainTile: nil, secondaryTiles: displayItems, speakerIndicatorSid: nil, pinnedIndicatorSid: nil),
            LayoutState(currentFocus: nil, focusHoldStartMs: nil, lastRemoteSpeakerSid: previousState.lastRemoteSpeakerSid)
        )
    }

    // Keep current state
    if let focus = previousState.currentFocus {
        let currentMain = displayItems.first {
            $0.participant.sid == focus.participantSid && $0.source == focus.source
        }
        if let currentMain = currentMain {
            let secondary = displayItems.filter { $0.id != currentMain.id }
            return (
                LayoutDecision(mode: .focus, mainTile: currentMain, secondaryTiles: secondary, speakerIndicatorSid: nil, pinnedIndicatorSid: nil),
                previousState
            )
        }
    }

    // Default: grid
    return (
        LayoutDecision(mode: .grid, mainTile: nil, secondaryTiles: displayItems, speakerIndicatorSid: nil, pinnedIndicatorSid: nil),
        LayoutState(currentFocus: nil, focusHoldStartMs: previousState.focusHoldStartMs, lastRemoteSpeakerSid: previousState.lastRemoteSpeakerSid)
    )
}
