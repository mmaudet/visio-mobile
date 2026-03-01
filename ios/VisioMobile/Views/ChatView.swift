import SwiftUI

struct ChatView: View {
    @EnvironmentObject private var manager: VisioManager
    @Environment(\.dismiss) private var dismiss

    @State private var messageText: String = ""

    var body: some View {
        VStack(spacing: 0) {
            // Messages list
            if manager.chatMessages.isEmpty {
                Spacer()
                Text("No messages yet")
                    .foregroundStyle(.secondary)
                Spacer()
            } else {
                ScrollViewReader { proxy in
                    ScrollView {
                        LazyVStack(alignment: .leading, spacing: 12) {
                            ForEach(manager.chatMessages, id: \.id) { message in
                                MessageBubble(message: message)
                                    .id(message.id)
                            }
                        }
                        .padding()
                    }
                    .onChange(of: manager.chatMessages.count) {
                        if let last = manager.chatMessages.last {
                            withAnimation {
                                proxy.scrollTo(last.id, anchor: .bottom)
                            }
                        }
                    }
                }
            }

            Divider()

            // Input bar
            HStack(spacing: 12) {
                TextField("Message", text: $messageText)
                    .textFieldStyle(.roundedBorder)
                    .onSubmit {
                        send()
                    }

                Button {
                    send()
                } label: {
                    Image(systemName: "arrow.up.circle.fill")
                        .font(.title2)
                        .foregroundStyle(.blue)
                }
                .disabled(messageText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 8)
        }
        .navigationTitle("Chat")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button("Done") {
                    dismiss()
                }
            }
        }
    }

    private func send() {
        let text = messageText
        messageText = ""
        manager.sendMessage(text)
    }
}

// MARK: - Message Bubble

private struct MessageBubble: View {
    let message: ChatMessage

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(message.senderName)
                    .font(.caption)
                    .fontWeight(.semibold)
                    .foregroundStyle(.blue)

                Spacer()

                Text(formattedTime)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }

            Text(message.text)
                .font(.body)
        }
        .padding(10)
        .background(Color(.systemGray6))
        .clipShape(RoundedRectangle(cornerRadius: 10))
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
