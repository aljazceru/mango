import SwiftUI
import Textual
import PhotosUI

/// Full chat screen: message thread + compose bar + header with model picker, attestation badge, and Instructions.
/// Per CHAT-01 through CHAT-14 and UI-SPEC interaction contract.
struct ChatView: View {
    @EnvironmentObject var appManager: AppManager
    let state: AppState
    @Binding var inputText: String
    let onSend: (BackendRole?) -> Void
    let onStop: () -> Void
    let onRetry: () -> Void
    let onEdit: (String, String) -> Void
    let onCopy: (String) -> Void
    let onAttach: (String, String, UInt64) -> Void
    let onClearAttachment: () -> Void
    let onSelectModel: (String) -> Void
    let onUseHybridProfile: (String) -> Void
    let onSetSystemPrompt: (String?) -> Void
    let onSetToolsEnabled: (Bool) -> Void
    let onRenameConversation: (String, String) -> Void
    let onBack: () -> Void
    // Phase 8: per-conversation document attachment (D-08)
    var onAttachDocument: (String) -> Void = { _ in }
    var onDetachDocument: (String) -> Void = { _ in }

    @State private var showSystemPromptSheet = false
    @State private var showFilePicker = false
    @State private var currentSystemPrompt: String = ""
    @State private var showDeleteConfirmation = false
    @State private var showDocAttachSheet = false
    @State private var showConvMenu = false
    @State private var showToolsSheet = false
    @State private var showRenameAlert = false
    @State private var renameText = ""
    @State private var forceRemoteNext = false
    // Phase 31 (IMG-05/IMG-06): attach action sheet + image pickers
    @State private var showAttachOptions = false
    @State private var showCameraPicker = false
    @State private var showPhotosPicker = false
    @State private var photosPickerItem: PhotosPickerItem? = nil

    var body: some View {
        VStack(spacing: 0) {
            // Tap-on-title rename affordance (only when a conversation exists).
            // Placed as a slim header strip above the thread so it's discoverable
            // without conflicting with the principal model picker in the nav bar.
            if let conv = currentConversation {
                HStack(spacing: 8) {
                    Button {
                        renameText = conv.title
                        showRenameAlert = true
                    } label: {
                        Text(conv.title)
                            .font(.subheadline)
                            .fontWeight(.medium)
                            .foregroundColor(.primary)
                            .lineLimit(1)
                            .frame(maxWidth: .infinity, alignment: .center)
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel("Rename conversation \(conv.title)")

                    Button {
                        showConvMenu = true
                    } label: {
                        Image(systemName: "ellipsis.circle")
                            .font(.title3)
                            .foregroundStyle(.secondary)
                            .frame(width: 32, height: 32)
                    }
                    .accessibilityLabel("Conversation options")
                    .accessibilityIdentifier("conversationOptionsButton")
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 6)
            }
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

            if let chip = hybridRouteChip {
                VStack(alignment: .leading, spacing: 2) {
                    Text(chip.label)
                        .font(.caption)
                        .fontWeight(.medium)
                    Text(chip.detail)
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 16)
                .padding(.vertical, 6)
                Divider()
            }

            // Compose bar
            ComposeBarView(
                inputText: $inputText,
                pendingAttachment: state.pendingAttachment,
                isStreaming: state.busyState.isStreaming,
                isInputBlocked: state.busyState.isBusy,
                showStopButton: state.busyState.isStreaming || state.busyState.isAttestationLoading,
                onSend: {
                    onSend(forceRemoteNext ? .remote : nil)
                    forceRemoteNext = false
                },
                onStop: onStop,
                onAttach: { showAttachOptions = true },
                onClearAttachment: onClearAttachment
            )
        }
        .navigationTitle(conversationTitle)
        .navigationBarTitleDisplayMode(.inline)
        .navigationBarBackButtonHidden(false)
        .onChange(of: currentConversation?.id) { _, _ in
            forceRemoteNext = false
        }
        .onChange(of: activeHybridProfileId) { _, _ in
            forceRemoteNext = false
        }
        .toolbar {
            ToolbarItemGroup(placement: .principal) {
                // Model picker with inline attestation indicator
                ModelPickerView(
                    backends: state.backends,
                    activeBackendId: state.activeBackendId,
                    selectedModelId: currentConversation?.modelId,
                    hybridProfiles: state.hybridProfiles,
                    activeHybridProfileId: activeHybridProfileId,
                    attestationStatus: activeAttestationStatus,
                    onSelectModel: onSelectModel,
                    onUseHybridProfile: onUseHybridProfile
                )
            }
            if activeHybridProfile != nil {
                ToolbarItem(placement: .primaryAction) {
                    Button(forceRemoteNext ? "Remote next: On" : "Remote next") {
                        forceRemoteNext.toggle()
                    }
                }
            }
        }
        .confirmationDialog("Conversation options", isPresented: $showConvMenu, titleVisibility: .visible) {
            let attachedCount = state.currentConversationAttachedDocs.count
            Button(attachedCount > 0 ? "RAG (\(attachedCount))" : "RAG") {
                showDocAttachSheet = true
            }

            Button("Instructions") {
                currentSystemPrompt = currentConversation?.systemPrompt ?? ""
                showSystemPromptSheet = true
            }

            let toolsOn = currentConversation?.toolsEnabled ?? false
            Button(toolsOn ? "Tools: On" : "Tools") {
                showToolsSheet = true
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
        // Phase 31 (IMG-05/IMG-06): paperclip opens action sheet with 3 choices.
        // Vision capability gating (follow-up to image-upload-still-broken-after-fix):
        // image entries are hidden when the selected model is not vision-capable so
        // the user never gets into the silent-failure path of sending a photo to a
        // text-only model.
        .confirmationDialog("Attach", isPresented: $showAttachOptions, titleVisibility: .hidden) {
            if currentModelSupportsVision {
                Button("Take Photo") { showCameraPicker = true }
                Button("Choose Photo") { showPhotosPicker = true }
            }
            Button("Attach File") { showFilePicker = true }
            Button("Cancel", role: .cancel) { }
        }
        .photosPicker(isPresented: $showPhotosPicker,
                      selection: $photosPickerItem,
                      matching: .images,
                      preferredItemEncoding: .compatible)
        .onChange(of: photosPickerItem) { _, item in
            guard let item else { return }
            Task {
                if let data = try? await item.loadTransferable(type: Data.self),
                   let ui = UIImage(data: data),
                   let jpeg = ui.jpegData(compressionQuality: 0.8) {
                    let url = FileManager.default.temporaryDirectory
                        .appendingPathComponent("gallery_\(UUID().uuidString).jpg")
                    try? jpeg.write(to: url)
                    await MainActor.run {
                        appManager.dispatch(.attachImage(filename: "image.jpg",
                                                         filePath: url.path,
                                                         mimeType: "image/jpeg"))
                    }
                }
                await MainActor.run { photosPickerItem = nil }
            }
        }
        .sheet(isPresented: $showCameraPicker) {
            ImagePickerView(
                onPicked: { filename, filePath, mimeType in
                    appManager.dispatch(.attachImage(filename: filename,
                                                     filePath: filePath,
                                                     mimeType: mimeType))
                    showCameraPicker = false
                },
                onCancel: { showCameraPicker = false }
            )
            .ignoresSafeArea()
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
        .sheet(isPresented: $showToolsSheet) {
            ToolsSheet(
                toolsEnabled: currentConversation?.toolsEnabled ?? false,
                braveApiKeySet: state.braveApiKeySet,
                onSetToolsEnabled: { enabled in
                    onSetToolsEnabled(enabled)
                },
                onDismiss: { showToolsSheet = false }
            )
        }
        .alert("Rename Conversation", isPresented: $showRenameAlert) {
            TextField("Conversation name", text: $renameText)
            Button("Save") {
                let trimmed = renameText.trimmingCharacters(in: .whitespacesAndNewlines)
                if let cid = currentConversation?.id, !trimmed.isEmpty {
                    onRenameConversation(cid, trimmed)
                }
            }
            Button("Cancel", role: .cancel) {}
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
        guard let backendId = activeHybridProfile?.remoteBackendId ?? state.activeBackendId else {
            return nil
        }
        return state.attestationStatuses.first(where: { $0.backendId == backendId })?.status
    }

    private var activeHybridProfileId: String? {
        let backendId = currentConversation?.backendId ?? state.activeBackendId
        guard let backendId, backendId.hasPrefix("hybrid:") else { return nil }
        return String(backendId.dropFirst("hybrid:".count))
    }

    private var activeHybridProfile: HybridProfile? {
        guard let activeHybridProfileId else { return nil }
        return state.hybridProfiles.first(where: { $0.id == activeHybridProfileId })
    }

    /// Whether the model selected for the current conversation supports vision
    /// inputs. Drives image-picker visibility in the attach action sheet.
    /// Defaults to false when no model is selected so the user is never offered
    /// an image option that would silently fail.
    private var currentModelSupportsVision: Bool {
        guard let modelId = activeHybridProfile?.remoteModelId ?? currentConversation?.modelId,
              !modelId.isEmpty else { return false }
        return modelSupportsVision(modelId: modelId)
    }

    private var hybridRouteChip: (label: String, detail: String)? {
        guard let profile = activeHybridProfile else { return nil }
        if forceRemoteNext {
            return (
                "Remote next turn · \(shortRouteModelName(profile.remoteModelId))",
                "Routing reason: user override"
            )
        }

        if let route = state.lastTurnRouting,
           route.profileId == profile.id,
           route.conversationId == state.currentConversationId {
            let label: String
            switch route.decision {
            case .local:
                label = "Answered locally · on-device"
            case .remote:
                label = route.teeVerified
                    ? "Escalated to \(route.providerName) · \(route.teeLabel) verified"
                    : "Escalated to \(route.providerName) · verifying"
            }
            return (label, "Routing reason: \(route.reason)")
        }

        return ("Hybrid ready · local by default", "Routing reason: local default")
    }

    private func shortRouteModelName(_ modelId: String) -> String {
        String(modelId.split(separator: "/").last ?? Substring(modelId))
            .replacingOccurrences(of: "_", with: " ")
            .replacingOccurrences(of: "-", with: " ")
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
                        onAttach(filename, content, sizeBytes)
                    }
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
                StructuredText(markdown: text)
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

// MARK: - Tools Sheet

/// Sheet for configuring per-conversation tool toggles (Phase 27, CHAT-TOOL-07).
/// Shows individual tool toggles so more tools can be added without UI changes.
private struct ToolsSheet: View {
    let toolsEnabled: Bool
    let braveApiKeySet: Bool
    let onSetToolsEnabled: (Bool) -> Void
    let onDismiss: () -> Void

    var body: some View {
        NavigationStack {
            List {
                Section {
                    Toggle(isOn: Binding(
                        get: { toolsEnabled },
                        set: { onSetToolsEnabled($0) }
                    )) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("Brave Search")
                                .font(.subheadline)
                            if !braveApiKeySet {
                                Text("API key not configured — set it in Settings")
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                        }
                    }
                    .disabled(!braveApiKeySet)
                } header: {
                    Text("Available Tools")
                } footer: {
                    Text("Tools let the assistant search the web and access other capabilities during this conversation.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
            .navigationTitle("Tools")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { onDismiss() }
                }
            }
        }
    }
}

// MARK: - BusyState Extension

private extension BusyState {
    var isBusy: Bool {
        switch self {
        case .idle:
            return false
        case .loading(_), .streaming(_):
            return true
        }
    }

    var isStreaming: Bool {
        if case .streaming = self { return true }
        return false
    }

    var isAttestationLoading: Bool {
        if case let .loading(message) = self {
            return message.localizedCaseInsensitiveContains("attestation")
        }
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
                                                    ? Color.accentColor : Color.secondary)
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
