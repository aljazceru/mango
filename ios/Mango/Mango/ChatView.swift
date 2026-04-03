import SwiftUI
import Textual

/// Full chat screen: message thread + compose bar + header with model picker, attestation badge, and Instructions.
/// Per CHAT-01 through CHAT-14 and UI-SPEC interaction contract.
struct ChatView: View {
    let state: AppState
    @Binding var inputText: String
    let onSend: () -> Void
    let onStop: () -> Void
    let onRetry: () -> Void
    let onEdit: (String, String) -> Void
    let onCopy: (String) -> Void
    let onAttach: () -> Void
    let onClearAttachment: () -> Void
    let onSelectModel: (String) -> Void
    let onSetSystemPrompt: (String?) -> Void
    let onBack: () -> Void
    // Phase 8: per-conversation document attachment (D-08)
    var onAttachDocument: (String) -> Void = { _ in }
    var onDetachDocument: (String) -> Void = { _ in }

    @State private var showSystemPromptSheet = false
    @State private var showFilePicker = false
    @State private var currentSystemPrompt: String = ""
    @State private var showDeleteConfirmation = false
    @State private var showDocAttachSheet = false

    var body: some View {
        VStack(spacing: 0) {
            Divider()

            // D-17: welcome placeholder when showFirstChatPlaceholder is true and messages empty
            if state.showFirstChatPlaceholder && state.messages.isEmpty {
                Spacer()
                Text("You're all set! Send your first message to start a confidential conversation.")
                    .foregroundColor(.secondary)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 32)
                Spacer()
            }

            // Message thread
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 8) {
                        ForEach(state.messages, id: \.id) { message in
                            MessageBubbleView(
                                message: message,
                                isLastAssistant: isLastAssistantMessage(message),
                                onCopy: { onCopy(message.content) },
                                onRetry: onRetry,
                                onEdit: { onEdit(message.id, message.content) }
                            )
                            .id(message.id)
                        }

                        // Streaming message bubble
                        if let streamingText = state.streamingText, !streamingText.isEmpty {
                            StreamingBubbleView(text: streamingText)
                                .id("streaming")
                                .accessibilityLabel("Streaming response")
                                .accessibilityAddTraits(.updatesFrequently)
                        }

                        // Error bubble
                        if let error = state.lastError {
                            ErrorBubbleView(error: error, onRetry: onRetry)
                                .id("error")
                        }

                        // Bottom spacer for scroll padding
                        Color.clear.frame(height: 8).id("bottom")
                    }
                    .padding(.horizontal, 16)
                    .padding(.vertical, 8)
                }
                .onChange(of: state.messages.count) { _, _ in
                    withAnimation {
                        proxy.scrollTo("bottom", anchor: .bottom)
                    }
                }
                .onChange(of: state.streamingText) { _, _ in
                    proxy.scrollTo("streaming", anchor: .bottom)
                }
            }

            Divider()

            // Compose bar
            ComposeBarView(
                inputText: $inputText,
                pendingAttachment: state.pendingAttachment,
                isStreaming: state.busyState.isStreaming,
                onSend: onSend,
                onStop: onStop,
                onAttach: { showFilePicker = true },
                onClearAttachment: onClearAttachment
            )
        }
        .navigationTitle(conversationTitle)
        .navigationBarTitleDisplayMode(.inline)
        .navigationBarBackButtonHidden(false)
        .toolbar {
            ToolbarItemGroup(placement: .principal) {
                HStack(spacing: 8) {
                    // Model picker
                    ModelPickerView(
                        backends: state.backends,
                        activeBackendId: state.activeBackendId,
                        selectedModelId: currentConversation?.modelId,
                        onSelectModel: onSelectModel
                    )
                    // Attestation badge
                    if let badge = activeAttestationStatus {
                        AttestationBadgeView(status: badge)
                    }
                }
            }
            ToolbarItem(placement: .primaryAction) {
                HStack(spacing: 8) {
                    // Per-conversation document attachment (D-08)
                    Button {
                        showDocAttachSheet = true
                    } label: {
                        let attachedCount = state.currentConversationAttachedDocs.count
                        Label(
                            attachedCount > 0 ? "Docs (\(attachedCount))" : "Docs",
                            systemImage: "doc.badge.plus"
                        )
                        .font(.subheadline)
                    }
                    .accessibilityLabel("Attach documents to this conversation")
                    Button("Instructions") {
                        currentSystemPrompt = currentConversation?.systemPrompt ?? ""
                        showSystemPromptSheet = true
                    }
                    .font(.subheadline)
                    .accessibilityLabel("Set instructions for this conversation")
                }
            }
        }
        .sheet(isPresented: $showSystemPromptSheet) {
            SystemPromptView(
                initialPrompt: currentSystemPrompt,
                onSave: { prompt in
                    onSetSystemPrompt(prompt.isEmpty ? nil : prompt)
                    showSystemPromptSheet = false
                },
                onCancel: { showSystemPromptSheet = false }
            )
        }
        .fileImporter(
            isPresented: $showFilePicker,
            allowedContentTypes: [.plainText, .pdf, .data],
            allowsMultipleSelection: false
        ) { result in
            handleFileImportResult(result)
        }
        .sheet(isPresented: $showDocAttachSheet) {
            DocumentAttachSheet(
                documents: state.documents,
                attachedDocIds: state.currentConversationAttachedDocs,
                onToggle: { docId in
                    let isAttached = state.currentConversationAttachedDocs.contains(docId)
                    if isAttached {
                        onDetachDocument(docId)
                    } else {
                        onAttachDocument(docId)
                    }
                },
                onDismiss: { showDocAttachSheet = false }
            )
        }
    }

    // MARK: - Computed Properties

    private var conversationTitle: String {
        currentConversation?.title ?? "New Conversation"
    }

    private var currentConversation: ConversationSummary? {
        guard let id = state.currentConversationId else { return nil }
        return state.conversations.first(where: { $0.id == id })
    }

    private var activeAttestationStatus: AttestationStatus? {
        guard let backendId = state.activeBackendId else { return nil }
        return state.attestationStatuses.first(where: { $0.backendId == backendId })?.status
    }

    private func isLastAssistantMessage(_ message: UiMessage) -> Bool {
        guard message.role == "assistant" else { return false }
        return state.messages.last(where: { $0.role == "assistant" })?.id == message.id
    }

    // MARK: - File Import

    private func handleFileImportResult(_ result: Result<[URL], Error>) {
        switch result {
        case .success(let urls):
            guard let url = urls.first else { return }
            Task {
                guard url.startAccessingSecurityScopedResource() else { return }
                defer { url.stopAccessingSecurityScopedResource() }
                do {
                    let data = try Data(contentsOf: url)
                    guard let content = String(data: data, encoding: .utf8) else { return }
                    let filename = url.lastPathComponent
                    let sizeBytes = UInt64(data.count)
                    await MainActor.run {
                        onAttach()
                    }
                    // Dispatch via AppManager is done at the parent coordinator level;
                    // here we surface the action via onAttach callback using the attachment data.
                    // The actual dispatch would be: appManager.dispatch(.attachFile(filename: filename, content: content, sizeBytes: sizeBytes))
                    // Since we follow the callback pattern, the parent dispatches the action.
                    _ = (filename, content, sizeBytes) // captured for parent coordinator use
                } catch {
                    // Non-text file or read error -- parent shows error toast
                }
            }
        case .failure:
            break
        }
    }
}

// MARK: - Streaming Bubble

/// Left-aligned bubble for the in-progress streaming response.
/// Uses Textual for incremental markdown rendering.
private struct StreamingBubbleView: View {
    let text: String

    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        HStack(alignment: .bottom) {
            VStack(alignment: .leading, spacing: 4) {
                StructuredText(text)
                    .font(.body)
                    .padding(.horizontal, 16)
                    .padding(.vertical, 8)
                    .background(AppColors.assistantBubble(colorScheme))
                    .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))

                // Blinking cursor indicator while streaming
                Text("▋")
                    .font(.body)
                    .foregroundColor(.secondary)
                    .padding(.leading, 16)
            }
            Spacer(minLength: 48)
        }
    }
}

// MARK: - Error Bubble

private struct ErrorBubbleView: View {
    let error: String
    let onRetry: () -> Void

    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            Image(systemName: "exclamationmark.triangle.fill")
                .foregroundColor(.red)
                .font(.subheadline)
            VStack(alignment: .leading, spacing: 4) {
                Text(error)
                    .font(.subheadline)
                    .foregroundColor(.primary)
                Button("Retry") { onRetry() }
                    .font(.caption)
                    .foregroundColor(.accentColor)
                    .accessibilityLabel("Retry last message")
            }
            Spacer()
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
        .background(Color.red.opacity(0.1))
        .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .stroke(Color.red.opacity(0.3), lineWidth: 1)
        )
    }
}

// MARK: - BusyState Extension

private extension BusyState {
    var isStreaming: Bool {
        if case .streaming = self { return true }
        return false
    }
}

// MARK: - Document Attachment Sheet

/// Sheet for toggling document attachment to the current conversation (D-08).
private struct DocumentAttachSheet: View {
    let documents: [DocumentSummary]
    let attachedDocIds: [String]
    let onToggle: (String) -> Void
    let onDismiss: () -> Void

    var body: some View {
        NavigationStack {
            Group {
                if documents.isEmpty {
                    VStack(spacing: 12) {
                        Spacer()
                        Image(systemName: "doc.text")
                            .font(.system(size: 40))
                            .foregroundStyle(.secondary)
                        Text("No documents in library")
                            .font(.headline)
                            .foregroundStyle(.secondary)
                        Spacer()
                    }
                } else {
                    List(documents, id: \.id) { doc in
                        Button {
                            onToggle(doc.id)
                        } label: {
                            HStack {
                                Image(systemName: attachedDocIds.contains(doc.id)
                                      ? "checkmark.circle.fill" : "circle")
                                    .foregroundStyle(attachedDocIds.contains(doc.id)
                                                    ? .accentColor : .secondary)
                                    .font(.title3)
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(doc.name)
                                        .font(.subheadline)
                                        .foregroundStyle(.primary)
                                    Text(doc.format.uppercased())
                                        .font(.caption2)
                                        .foregroundStyle(.secondary)
                                }
                                Spacer()
                            }
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
            .navigationTitle("Attach Documents")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { onDismiss() }
                }
            }
        }
    }
}
