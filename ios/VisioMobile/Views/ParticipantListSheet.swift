import SwiftUI
import visioFFI

struct ParticipantListSheet: View {
    @EnvironmentObject private var manager: VisioManager
    @Environment(\.dismiss) private var dismiss

    private var lang: String { manager.currentLang }
    private var isDark: Bool { manager.currentTheme == "dark" }

    var body: some View {
        NavigationStack {
            List {
                // Local participant (always first)
                localParticipantRow

                // Remote participants sorted: hand raised first, then alphabetical
                let sorted = sortedParticipants
                ForEach(sorted, id: \.sid) { p in
                    participantRow(p)
                }
            }
            .scrollContentBackground(.hidden)
            .background(VisioColors.background(dark: isDark))
            .navigationTitle("\(Strings.t("participants.title", lang: lang)) (\(manager.participants.count + 1))")
            .navigationBarTitleDisplayMode(.inline)
            .toolbarColorScheme(isDark ? .dark : .light, for: .navigationBar)
            .toolbarBackground(VisioColors.surface(dark: isDark), for: .navigationBar)
            .toolbarBackground(.visible, for: .navigationBar)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button {
                        dismiss()
                    } label: {
                        Image(systemName: "xmark")
                            .foregroundStyle(VisioColors.onSurface(dark: isDark))
                    }
                }
            }
        }
    }

    // MARK: - Local Participant Row

    private var localParticipantRow: some View {
        let name = manager.displayName.isEmpty
            ? Strings.t("call.you", lang: lang)
            : manager.displayName

        return HStack(spacing: 12) {
            avatarCircle(name: name)

            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 4) {
                    Text(name)
                        .font(.body)
                        .foregroundStyle(VisioColors.onSurface(dark: isDark))
                        .lineLimit(1)
                    Text("(\(Strings.t("call.you", lang: lang)))")
                        .font(.caption)
                        .foregroundStyle(VisioColors.secondaryText(dark: isDark))
                }
            }

            Spacer()

            statusIcons(
                isMuted: !manager.isMicEnabled,
                hasVideo: manager.isCameraEnabled,
                handRaisePosition: manager.isHandRaised ? 1 : 0,
                quality: .excellent
            )
        }
        .listRowBackground(VisioColors.surface(dark: isDark))
    }

    // MARK: - Remote Participant Row

    private func participantRow(_ p: ParticipantInfo) -> some View {
        let name = p.name ?? p.identity

        return HStack(spacing: 12) {
            avatarCircle(name: name)

            Text(name)
                .font(.body)
                .foregroundStyle(VisioColors.onSurface(dark: isDark))
                .lineLimit(1)

            Spacer()

            statusIcons(
                isMuted: p.isMuted,
                hasVideo: p.hasVideo,
                handRaisePosition: manager.handRaisedMap[p.sid] ?? 0,
                quality: p.connectionQuality
            )
        }
        .listRowBackground(VisioColors.surface(dark: isDark))
    }

    // MARK: - Shared Components

    private func avatarCircle(name: String) -> some View {
        let initials: String = {
            let parts = name.split(separator: " ")
            if parts.count >= 2 {
                return String(parts[0].prefix(1) + parts[1].prefix(1)).uppercased()
            }
            return String(name.prefix(2)).uppercased()
        }()

        let hue = Double(name.unicodeScalars.reduce(0) { $0 + Int($1.value) } % 360) / 360.0

        return ZStack {
            Circle()
                .fill(Color(hue: hue, saturation: 0.5, brightness: 0.35))
                .frame(width: 40, height: 40)
            Text(initials)
                .font(.system(size: 16, weight: .bold))
                .foregroundStyle(.white)
        }
    }

    private func statusIcons(
        isMuted: Bool,
        hasVideo: Bool,
        handRaisePosition: Int,
        quality: ConnectionQuality
    ) -> some View {
        HStack(spacing: 6) {
            if isMuted {
                Image(systemName: "mic.slash.fill")
                    .font(.system(size: 14))
                    .foregroundStyle(VisioColors.error500)
            }

            if !hasVideo {
                Image(systemName: "video.slash.fill")
                    .font(.system(size: 14))
                    .foregroundStyle(VisioColors.error500)
            }

            if handRaisePosition > 0 {
                HStack(spacing: 2) {
                    Image(systemName: "hand.raised.fill")
                        .font(.system(size: 11))
                    Text("\(handRaisePosition)")
                        .font(.caption2)
                        .bold()
                }
                .padding(.horizontal, 5)
                .padding(.vertical, 2)
                .background(VisioColors.handRaise)
                .clipShape(Capsule())
                .foregroundStyle(.black)
            }

            qualityIndicator(quality)
        }
    }

    @ViewBuilder
    private func qualityIndicator(_ quality: ConnectionQuality) -> some View {
        switch quality {
        case .excellent:
            Image(systemName: "wifi")
                .font(.system(size: 10))
                .foregroundStyle(.green)
        case .good:
            Image(systemName: "wifi")
                .font(.system(size: 10))
                .foregroundStyle(.yellow)
        case .poor:
            Image(systemName: "wifi.exclamationmark")
                .font(.system(size: 10))
                .foregroundStyle(.orange)
        case .lost:
            Image(systemName: "wifi.slash")
                .font(.system(size: 10))
                .foregroundStyle(VisioColors.error500)
        }
    }

    // MARK: - Sorting

    private var sortedParticipants: [ParticipantInfo] {
        manager.participants.sorted { a, b in
            let aRaised = manager.handRaisedMap[a.sid]
            let bRaised = manager.handRaisedMap[b.sid]

            // Hand raised first
            if aRaised != nil && bRaised == nil { return true }
            if aRaised == nil && bRaised != nil { return false }
            if let ap = aRaised, let bp = bRaised {
                if ap != bp { return ap < bp }
            }

            // Then alphabetical
            let aName = (a.name ?? a.identity).lowercased()
            let bName = (b.name ?? b.identity).lowercased()
            return aName < bName
        }
    }
}
