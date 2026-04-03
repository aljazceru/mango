import SwiftUI

struct ConversationListView: View {
    let state: AppState
    let onSelect: (String) -> Void
    let onNew: () -> Void
    let onDelete: (String) -> Void
    let onRename: (String, String) -> Void

    @State private var renameTarget: ConversationSummary? = nil
    @State private var renameText: String = ""
    @State private var showRenameAlert = false

    var body: some View {
        NavigationStack {
            List {
                ForEach(state.conversations, id: \.id) { conversation in
                    Button(action: { onSelect(conversation.id) }) {
                        VStack(alignment: .leading, spacing: 4) {
                            Text(conversation.title)
                                .font(.body)
                                .lineLimit(1)
                                .foregroundColor(.primary)
                            HStack(spacing: 4) {
                                Text(relativeTime(conversation.updatedAt))
                                    .font(.subheadline)
                                    .foregroundColor(.secondary)
                                Text("·")
                                    .foregroundColor(.secondary)
                                Text(shortModelName(conversation.modelId))
                                    .font(.subheadline)
                                    .foregroundColor(.secondary)
                            }
                        }
                        .padding(.vertical, 4)
                    }
                    .buttonStyle(.plain)
                    .swipeActions(edge: .trailing) {
                        Button(role: .destructive) {
                            onDelete(conversation.id)
                        } label: {
                            Label("Delete", systemImage: "trash")
                        }
                    }
                    .contextMenu {
                        Button("Rename") {
                            renameTarget = conversation
                            renameText = conversation.title
                            showRenameAlert = true
                        }
                        Button("Delete", role: .destructive) {
                            onDelete(conversation.id)
                        }
                    }
                }
            }
            .navigationTitle("Conversations")
            .toolbar {
                ToolbarItem(placement: .primaryAction) {
                    Button(action: onNew) {
                        Label("New Conversation", systemImage: "plus")
                    }
                    .accessibilityLabel("New Conversation")
                }
            }
            .overlay {
                if state.conversations.isEmpty {
                    VStack(spacing: 16) {
                        Text("No conversations yet")
                            .font(.title3)
                            .fontWeight(.semibold)
                        Text("Start a new conversation to chat with a private AI.")
                            .font(.body)
                            .foregroundColor(.secondary)
                            .multilineTextAlignment(.center)
                        Button("New Conversation", action: onNew)
                            .buttonStyle(.borderedProminent)
                            .accessibilityLabel("New Conversation")
                    }
                    .padding(.horizontal, 32)
                }
            }
            .alert("Rename Conversation", isPresented: $showRenameAlert) {
                TextField("Conversation name", text: $renameText)
                Button("Save") {
                    if let target = renameTarget, !renameText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                        onRename(target.id, renameText)
                    }
                    renameTarget = nil
                }
                Button("Cancel", role: .cancel) {
                    renameTarget = nil
                }
            }
        }
    }
}

// MARK: - Helpers

private func relativeTime(_ epochMillis: Int64) -> String {
    let date = Date(timeIntervalSince1970: Double(epochMillis) / 1000.0)
    let formatter = RelativeDateTimeFormatter()
    formatter.unitsStyle = .short
    return formatter.localizedString(for: date, relativeTo: Date())
}

private func shortModelName(_ modelId: String) -> String {
    // Strip provider prefix: "openai/gpt-4o" -> "gpt-4o"
    if let slash = modelId.lastIndex(of: "/") {
        return String(modelId[modelId.index(after: slash)...])
    }
    return modelId
}
