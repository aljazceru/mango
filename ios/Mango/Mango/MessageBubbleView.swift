import SwiftUI
import Textual

/// Renders a single chat message bubble.
/// User messages: right-aligned, blue tint, plain text.
/// Assistant messages: left-aligned, surface grey, markdown via Textual.
struct MessageBubbleView: View {
    let message: UiMessage
    let isLastAssistant: Bool
    let onCopy: () -> Void
    let onRetry: () -> Void
    let onEdit: () -> Void

    @Environment(\.colorScheme) private var colorScheme
    @EnvironmentObject private var appManager: AppManager

    // IMG-07: thumbnail loaded on-demand via decrypt-on-read
    @State private var thumbnail: UIImage? = nil

    var body: some View {
        Group {
            if message.role == "user" {
                userBubble
            } else if message.role == "assistant" {
                assistantBubble
            } else {
                // system role: show dimmed
                systemBubble
            }
        }
        .task(id: message.id) {
            guard message.imagePath != nil else { return }
            do {
                let bytes = try appManager.ffiApp.readEncryptedImage(messageId: message.id)
                if let img = UIImage(data: Data(bytes)) {
                    await MainActor.run { thumbnail = img }
                }
            } catch {
                // Decryption failure: thumbnail stays nil, text content shown as fallback
            }
        }
    }

    // MARK: - User Bubble

    private var userBubble: some View {
        HStack(alignment: .bottom) {
            Spacer(minLength: 48)
            VStack(alignment: .trailing, spacing: 4) {
                if let name = message.attachmentName, message.hasAttachment {
                    attachmentIndicator(name: name)
                }
                // IMG-07: show decrypted thumbnail if available, fall through to text otherwise
                if let thumb = thumbnail {
                    Image(uiImage: thumb)
                        .resizable()
                        .scaledToFit()
                        .frame(maxWidth: 240, maxHeight: 240)
                        .cornerRadius(8)
                        .accessibilityLabel("Sent image")
                }
                if thumbnail == nil || !message.content.isEmpty {
                    Text(message.content)
                        .font(.body)
                        .foregroundColor(colorScheme == .dark ? .white : .black)
                        .padding(.horizontal, 16)
                        .padding(.vertical, 8)
                        .background(userBubbleColor)
                        .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
                        .textSelection(.enabled)
                        .accessibilityAddTraits(.isStaticText)
                }
                // Action row: edit
                HStack(spacing: 8) {
                    Button("Edit") { onEdit() }
                        .font(.caption)
                        .foregroundColor(.secondary)
                        .accessibilityLabel("Edit message")
                    Button("Copy") { onCopy() }
                        .font(.caption)
                        .foregroundColor(.secondary)
                        .accessibilityLabel("Copy message to clipboard")
                }
            }
        }
    }

    // MARK: - Assistant Bubble

    private var assistantBubble: some View {
        HStack(alignment: .bottom) {
            VStack(alignment: .leading, spacing: 4) {
                if let name = message.attachmentName, message.hasAttachment {
                    attachmentIndicator(name: name)
                }
                StructuredText(markdown: message.content)
                .font(.body)
                .padding(.horizontal, 16)
                .padding(.vertical, 8)
                .background(assistantBubbleColor)
                .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
                .accessibilityAddTraits(.updatesFrequently)
                // RAG context indicator (D-07): shown when documents contributed context
                if let ragCount = message.ragContextCount, ragCount > 0 {
                    Text("[context from \(ragCount) doc(s)]")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .padding(.leading, 4)
                }
                // Action row: copy, retry
                HStack(spacing: 8) {
                    Button("Copy") { onCopy() }
                        .font(.caption)
                        .foregroundColor(.secondary)
                        .accessibilityLabel("Copy message to clipboard")
                    if isLastAssistant {
                        Button("Retry") { onRetry() }
                            .font(.caption)
                            .foregroundColor(.secondary)
                            .accessibilityLabel("Retry last message")
                    }
                }
            }
            Spacer(minLength: 48)
        }
    }

    // MARK: - System Bubble

    private var systemBubble: some View {
        HStack {
            Spacer()
            Text(message.content)
                .font(.caption)
                .foregroundColor(.secondary)
                .italic()
                .padding(.horizontal, 16)
                .padding(.vertical, 4)
            Spacer()
        }
    }

    // MARK: - Attachment Indicator

    private func attachmentIndicator(name: String) -> some View {
        HStack(spacing: 4) {
            Image(systemName: "paperclip")
                .font(.caption)
                .foregroundColor(.secondary)
            Text(name)
                .font(.caption)
                .foregroundColor(.secondary)
                .lineLimit(1)
        }
    }

    // MARK: - Colors

    private var userBubbleColor: Color {
        AppColors.userBubble(colorScheme)
    }

    private var assistantBubbleColor: Color {
        AppColors.assistantBubble(colorScheme)
    }
}
