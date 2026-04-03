import SwiftUI

/// Sheet for editing the system prompt ("Instructions") for a conversation.
/// Per CHAT-11 / D-09: accessible from the chat header as "Instructions".
struct SystemPromptView: View {
    let initialPrompt: String
    let onSave: (String) -> Void
    let onCancel: () -> Void

    @State private var promptText: String

    init(initialPrompt: String = "", onSave: @escaping (String) -> Void, onCancel: @escaping () -> Void) {
        self.initialPrompt = initialPrompt
        self.onSave = onSave
        self.onCancel = onCancel
        _promptText = State(initialValue: initialPrompt)
    }

    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 12) {
                Text("Instructions")
                    .font(.title3)
                    .fontWeight(.semibold)

                Text("Optional: give the assistant a role or set of instructions.")
                    .font(.subheadline)
                    .foregroundColor(.secondary)

                TextField(
                    "Optional: give the assistant a role or set of instructions.",
                    text: $promptText,
                    axis: .vertical
                )
                .textFieldStyle(.roundedBorder)
                .lineLimit(3...10)
                .font(.body)
                .accessibilityLabel("System prompt instructions")

                Spacer()
            }
            .padding(16)
            .navigationTitle("Instructions")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel", action: onCancel)
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") {
                        onSave(promptText)
                    }
                }
            }
        }
    }
}
