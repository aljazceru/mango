import SwiftUI

/// Compose bar pinned to the bottom of the chat screen.
/// Includes text input, file attachment indicator, send/stop buttons.
/// Uses .safeAreaInset to stay above the keyboard.
struct ComposeBarView: View {
    @Binding var inputText: String
    let pendingAttachment: AttachmentInfo?
    let isStreaming: Bool
    let onSend: () -> Void
    let onStop: () -> Void
    let onAttach: () -> Void
    let onClearAttachment: () -> Void

    var body: some View {
        VStack(spacing: 4) {
            // Pending attachment indicator
            if let attachment = pendingAttachment {
                HStack(spacing: 8) {
                    Image(systemName: "paperclip")
                        .foregroundColor(.secondary)
                        .font(.subheadline)
                    Text("\(attachment.filename) (\(attachment.sizeDisplay))")
                        .font(.subheadline)
                        .foregroundColor(.secondary)
                        .lineLimit(1)
                    Spacer()
                    Button(action: onClearAttachment) {
                        Image(systemName: "xmark.circle.fill")
                            .foregroundColor(.secondary)
                    }
                    .accessibilityLabel("Remove attachment")
                }
                .padding(.horizontal, 16)
                .padding(.top, 8)
            }

            HStack(alignment: .bottom, spacing: 8) {
                // Attach button
                Button(action: onAttach) {
                    Image(systemName: "paperclip")
                        .font(.title3)
                        .foregroundColor(.secondary)
                }
                .accessibilityLabel("Attach file for context")
                .frame(minWidth: 44, minHeight: 44)

                // Text input
                TextField("Message", text: $inputText, axis: .vertical)
                    .textFieldStyle(.roundedBorder)
                    .lineLimit(1...6)
                    .font(.body)
                    .disabled(isStreaming)
                    .onSubmit {
                        if !isStreaming && !inputText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                            onSend()
                        }
                    }

                // Send / Stop button
                if isStreaming {
                    Button(action: onStop) {
                        Image(systemName: "stop.fill")
                            .font(.title3)
                            .foregroundColor(.red)
                    }
                    .accessibilityLabel("Stop generating")
                    .frame(minWidth: 44, minHeight: 44)
                } else {
                    Button(action: onSend) {
                        Image(systemName: "arrow.up.circle.fill")
                            .font(.title2)
                            .foregroundColor(
                                inputText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                                    ? .secondary
                                    : .accentColor
                            )
                    }
                    .disabled(inputText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    .accessibilityLabel("Send message")
                    .frame(minWidth: 44, minHeight: 44)
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 8)
        }
        .background(.background)
        .safeAreaInset(edge: .bottom) { EmptyView() }
    }
}
