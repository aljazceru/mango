import SwiftUI

/// Memory Management screen: view, edit, and delete stored memories (MEM-04, MEM-05, MEM-06).
/// Memories are automatically extracted from conversations and stored locally.
struct MemoryManagementView: View {
    @EnvironmentObject var appManager: AppManager

    @State private var selectedMemoryId: String? = nil
    @State private var editText: String = ""
    @State private var memoryToDelete: MemorySummary? = nil
    @State private var showDeleteConfirmation: Bool = false

    var appState: AppState { appManager.appState }

    // MARK: - Body

    var body: some View {
        NavigationStack {
            Group {
                if appState.memories.isEmpty {
                    emptyStateView
                } else {
                    memoryListView
                }
            }
            .navigationTitle("Memories")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Back") {
                        appManager.dispatch(.popScreen)
                    }
                }
            }
            .confirmationDialog("Delete Memory?", isPresented: $showDeleteConfirmation, titleVisibility: .visible) {
                Button("Delete", role: .destructive) {
                    if let memory = memoryToDelete {
                        appManager.dispatch(.deleteMemory(memoryId: memory.id))
                    }
                    memoryToDelete = nil
                }
                Button("Cancel", role: .cancel) {
                    memoryToDelete = nil
                }
            } message: {
                Text("This memory will be permanently removed.")
            }
            .onAppear {
                appManager.dispatch(.listMemories)
            }
        }
    }

    // MARK: - Subviews

    private var emptyStateView: some View {
        VStack(spacing: 12) {
            Spacer()
            Image(systemName: "brain")
                .font(.system(size: 48))
                .foregroundStyle(.secondary)
            Text("No memories yet")
                .font(.headline)
                .foregroundStyle(.primary)
            Text("Memories are automatically extracted from your conversations.")
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)
            Spacer()
        }
    }

    private var memoryListView: some View {
        List {
            ForEach(appState.memories, id: \.id) { memory in
                memoryRow(memory)
            }
            .onDelete { indexSet in
                for index in indexSet {
                    memoryToDelete = appState.memories[index]
                    showDeleteConfirmation = true
                }
            }
        }
        .listStyle(.insetGrouped)
    }

    @ViewBuilder
    private func memoryRow(_ memory: MemorySummary) -> some View {
        if selectedMemoryId == memory.id {
            // Edit mode
            VStack(alignment: .leading, spacing: 8) {
                TextField("Memory content", text: $editText, axis: .vertical)
                    .font(.subheadline)
                    .lineLimit(3...10)
                    .textFieldStyle(.roundedBorder)
                HStack(spacing: 12) {
                    Button("Save") {
                        appManager.dispatch(.updateMemory(memoryId: memory.id, content: editText))
                        selectedMemoryId = nil
                    }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.small)
                    Button("Cancel") {
                        selectedMemoryId = nil
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                }
            }
            .padding(.vertical, 4)
        } else {
            // Display mode
            VStack(alignment: .leading, spacing: 4) {
                Text(memory.contentPreview)
                    .font(.subheadline)
                    .lineLimit(3)
                Text(memory.conversationTitle ?? "Unknown conversation")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Text(formatDate(memory.createdAt))
                    .font(.caption2)
                    .foregroundStyle(.tertiary)
            }
            .padding(.vertical, 2)
            .contentShape(Rectangle())
            .onTapGesture {
                selectedMemoryId = memory.id
                editText = memory.content
            }
        }
    }

    // MARK: - Helpers

    private func formatDate(_ unixTimestamp: Int64) -> String {
        let now = Int64(Date().timeIntervalSince1970)
        let diff = now - unixTimestamp
        if diff < 60 { return "just now" }
        if diff < 3600 { return "\(diff / 60)m ago" }
        if diff < 86400 { return "\(diff / 3600)h ago" }
        let days = diff / 86400
        if days == 1 { return "yesterday" }
        return "\(days)d ago"
    }
}
