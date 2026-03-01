import SwiftUI
import visioFFI

struct ChatView: View {
    @EnvironmentObject private var manager: VisioManager
    @Environment(\.dismiss) private var dismiss

    @State private var messageText: String = ""

    var body: some View {
        ZStack {
            VisioColors.primaryDark50.ignoresSafeArea()

            VStack(spacing: 0) {
                // Messages list
                if manager.chatMessages.isEmpty {
                    Spacer()
                    Text("No messages yet")
                        .foregroundStyle(VisioColors.greyscale400)
                    Spacer()
                } else {
                    ScrollViewReader { proxy in
                        ScrollView {
                            LazyVStack(spacing: 4) {
                                ForEach(Array(manager.chatMessages.enumerated()), id: \.element.id) { index, message in
                                    let showSender = shouldShowSender(at: index)
                                    let isOwn = isOwnMessage(message)
                                    MessageBubble(
                                        message: message,
                                        isOwn: isOwn,
                                        showSender: showSender
                                    )
                                    .id(message.id)
                                }
                            }
                            .padding()
                        }
                        .onChange(of: manager.chatMessages.count) { _ in
                            if let last = manager.chatMessages.last {
                                withAnimation {
                                    proxy.scrollTo(last.id, anchor: .bottom)
                                }
                            }
                        }
                    }
                }

                // Input bar
                HStack(spacing: 12) {
                    TextField("Message", text: $messageText)
                        .textFieldStyle(.plain)
                        .padding(.horizontal, 12)
                        .padding(.vertical, 8)
                        .background(VisioColors.primaryDark100)
                        .clipShape(RoundedRectangle(cornerRadius: 20))
                        .foregroundStyle(.white)
                        .onSubmit { send() }

                    Button {
                        send()
                    } label: {
                        Image(systemName: "paperplane.fill")
                            .font(.system(size: 18, weight: .medium))
                            .foregroundStyle(messageText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? VisioColors.greyscale400 : VisioColors.primary500)
                            .frame(width: 36, height: 36)
                    }
                    .disabled(messageText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 8)
                .background(VisioColors.primaryDark75)
            }
        }
        .navigationTitle("Chat")
        .navigationBarTitleDisplayMode(.inline)
        .toolbarColorScheme(.dark, for: .navigationBar)
        .toolbarBackground(VisioColors.primaryDark75, for: .navigationBar)
        .toolbarBackground(.visible, for: .navigationBar)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button {
                    dismiss()
                } label: {
                    Image(systemName: "xmark")
                        .foregroundStyle(VisioColors.greyscale400)
                }
            }
        }
        .preferredColorScheme(.dark)
    }

    private func send() {
        let text = messageText
        messageText = ""
        manager.sendMessage(text)
    }

    /// Determine if this message is from the local user.
    /// The first participant is typically the local participant.
    private func isOwnMessage(_ message: ChatMessage) -> Bool {
        guard let localParticipant = manager.participants.first else { return false }
        return message.senderSid == localParticipant.sid
    }

    /// Hide sender name if same sender as previous message within 60 seconds.
    private func shouldShowSender(at index: Int) -> Bool {
        guard index > 0 else { return true }
        let current = manager.chatMessages[index]
        let previous = manager.chatMessages[index - 1]
        if current.senderSid != previous.senderSid { return true }
        let diff = current.timestampMs - previous.timestampMs
        return diff > 60_000
    }
}

// MARK: - Message Bubble

private struct MessageBubble: View {
    let message: ChatMessage
    let isOwn: Bool
    let showSender: Bool

    var body: some View {
        VStack(alignment: isOwn ? .trailing : .leading, spacing: 2) {
            if showSender {
                HStack {
                    if isOwn { Spacer() }
                    Text(message.senderName)
                        .font(.caption)
                        .fontWeight(.semibold)
                        .foregroundStyle(VisioColors.greyscale400)
                    Text(formattedTime)
                        .font(.caption2)
                        .foregroundStyle(VisioColors.greyscale400.opacity(0.7))
                    if !isOwn { Spacer() }
                }
                .padding(.top, 8)
            }

            HStack {
                if isOwn { Spacer(minLength: 60) }
                Text(message.text)
                    .font(.body)
                    .foregroundStyle(.white)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 8)
                    .background(isOwn ? VisioColors.primary500 : VisioColors.primaryDark100)
                    .clipShape(RoundedRectangle(cornerRadius: 12))
                if !isOwn { Spacer(minLength: 60) }
            }
        }
        .frame(maxWidth: .infinity, alignment: isOwn ? .trailing : .leading)
    }

    private var formattedTime: String {
        let date = Date(timeIntervalSince1970: Double(message.timestampMs) / 1000.0)
        let formatter = DateFormatter()
        formatter.timeStyle = .short
        return formatter.string(from: date)
    }
}

#Preview {
    NavigationStack {
        ChatView()
            .environmentObject(VisioManager())
    }
}
