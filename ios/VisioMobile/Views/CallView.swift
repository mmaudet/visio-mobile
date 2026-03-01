import SwiftUI

struct CallView: View {
    @EnvironmentObject private var manager: VisioManager
    @Environment(\.dismiss) private var dismiss

    let roomURL: String
    let displayName: String

    @State private var showChat: Bool = false

    var body: some View {
        VStack(spacing: 0) {
            // Connection state banner
            connectionBanner

            // Error banner
            if let error = manager.errorMessage {
                Text(error)
                    .font(.caption)
                    .foregroundStyle(.white)
                    .padding(8)
                    .frame(maxWidth: .infinity)
                    .background(Color.red)
            }

            // Participants list
            if manager.participants.isEmpty {
                Spacer()
                VStack(spacing: 12) {
                    ProgressView()
                    Text("Waiting for participants...")
                        .foregroundStyle(.secondary)
                }
                Spacer()
            } else {
                List(manager.participants, id: \.sid) { participant in
                    ParticipantRow(participant: participant)
                }
                .listStyle(.plain)
            }

            // Bottom toolbar
            bottomToolbar
        }
        .navigationTitle("Call")
        .navigationBarTitleDisplayMode(.inline)
        .navigationBarBackButtonHidden(true)
        .sheet(isPresented: $showChat) {
            NavigationStack {
                ChatView()
                    .environmentObject(manager)
            }
        }
        .onAppear {
            let name = displayName.isEmpty ? nil : displayName
            manager.connect(url: roomURL, username: name)
        }
    }

    // MARK: - Connection Banner

    @ViewBuilder
    private var connectionBanner: some View {
        switch manager.connectionState {
        case .connecting:
            bannerView(text: "Connecting...", color: .orange)
        case .reconnecting(let attempt):
            bannerView(text: "Reconnecting (attempt \(attempt))...", color: .orange)
        case .disconnected:
            bannerView(text: "Disconnected", color: .gray)
        case .connected:
            EmptyView()
        }
    }

    private func bannerView(text: String, color: Color) -> some View {
        Text(text)
            .font(.caption)
            .fontWeight(.medium)
            .foregroundStyle(.white)
            .padding(6)
            .frame(maxWidth: .infinity)
            .background(color)
    }

    // MARK: - Bottom Toolbar

    private var bottomToolbar: some View {
        HStack(spacing: 24) {
            // Mic toggle
            Button {
                manager.toggleMic()
            } label: {
                Image(systemName: manager.isMicEnabled ? "mic.fill" : "mic.slash.fill")
                    .font(.title2)
                    .foregroundStyle(manager.isMicEnabled ? .blue : .red)
            }

            // Camera toggle
            Button {
                manager.toggleCamera()
            } label: {
                Image(systemName: manager.isCameraEnabled ? "video.fill" : "video.slash.fill")
                    .font(.title2)
                    .foregroundStyle(manager.isCameraEnabled ? .blue : .red)
            }

            // Chat
            Button {
                showChat = true
            } label: {
                Image(systemName: "message.fill")
                    .font(.title2)
                    .foregroundStyle(.blue)
            }

            // Hang up
            Button {
                manager.disconnect()
                dismiss()
            } label: {
                Image(systemName: "phone.down.fill")
                    .font(.title2)
                    .foregroundStyle(.white)
                    .padding(12)
                    .background(Color.red, in: Circle())
            }
        }
        .padding(.vertical, 12)
        .padding(.horizontal, 24)
        .background(.ultraThinMaterial)
    }
}

// MARK: - Participant Row

private struct ParticipantRow: View {
    let participant: ParticipantInfo

    var body: some View {
        HStack {
            Image(systemName: "person.fill")
                .foregroundStyle(.secondary)

            Text(participant.name ?? participant.identity)
                .font(.body)

            Spacer()

            // Connection quality indicator
            connectionQualityIcon

            // Mute indicator
            if participant.isMuted {
                Image(systemName: "mic.slash.fill")
                    .foregroundStyle(.red)
                    .font(.caption)
            }

            // Video indicator
            if participant.hasVideo {
                Image(systemName: "video.fill")
                    .foregroundStyle(.green)
                    .font(.caption)
            }
        }
    }

    @ViewBuilder
    private var connectionQualityIcon: some View {
        switch participant.connectionQuality {
        case .excellent:
            Image(systemName: "wifi")
                .foregroundStyle(.green)
                .font(.caption)
        case .good:
            Image(systemName: "wifi")
                .foregroundStyle(.yellow)
                .font(.caption)
        case .poor:
            Image(systemName: "wifi.exclamationmark")
                .foregroundStyle(.orange)
                .font(.caption)
        case .lost:
            Image(systemName: "wifi.slash")
                .foregroundStyle(.red)
                .font(.caption)
        }
    }
}

#Preview {
    NavigationStack {
        CallView(roomURL: "meet.example.com/test", displayName: "Alice")
            .environmentObject(VisioManager())
    }
}
