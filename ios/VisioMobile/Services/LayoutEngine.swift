import Foundation
import visioFFI

// MARK: - Layout Engine Types

enum LayoutMode {
    case grid
    case focus
    case speaker
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

    // 2. Pin has priority — user explicitly pinned a participant
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

    // 3. Default: grid layout (no auto-focus on active speaker)
    return (
        LayoutDecision(mode: .grid, mainTile: nil, secondaryTiles: displayItems, speakerIndicatorSid: activeSpeakers.first, pinnedIndicatorSid: nil),
        LayoutState(currentFocus: nil, focusHoldStartMs: nil, lastRemoteSpeakerSid: previousState.lastRemoteSpeakerSid)
    )
}
