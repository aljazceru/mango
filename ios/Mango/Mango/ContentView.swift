import SwiftUI
import UIKit

/// Root content view: routes to Settings, Chat, or Home based on router state.
struct ContentView: View {
    @EnvironmentObject var appManager: AppManager
    @Environment(\.scenePhase) var scenePhase

    /// Timestamp when the app last moved to background (D-10).
    @State private var backgroundedAt: Date? = nil
    @State private var chatInputText: String = ""

    var body: some View {
        let isReady = appManager.isReady
        let screen = appManager.appState.router.currentScreen
        Group {
            if !isReady {
                Color(.systemBackground)
                    .ignoresSafeArea()
            } else {
                switch screen {
            case .locked:
                LockScreen()
                    .environmentObject(appManager)
            case .pinSetup:
                PinSetupScreen()
                    .environmentObject(appManager)
            case .onboarding(let step):
                OnboardingView(step: step)
                    .environmentObject(appManager)
            case .settings:
                SettingsView()
                    .environmentObject(appManager)
            case .settingsLocalModels, .settingsHybridRouting:
                SettingsView()
                    .environmentObject(appManager)
            case .documents:
                DocumentLibraryView()
                    .environmentObject(appManager)
            case .directorySources:
                DirectorySourcesView()
                    .environmentObject(appManager)
            case .memories:
                MemoryManagementView()
                    .environmentObject(appManager)
            case .settingsProviders:
                SettingsProvidersView()
                    .environmentObject(appManager)
            case .settingsDefaults:
                SettingsDefaultsView()
                    .environmentObject(appManager)
            case .settingsMemory:
                SettingsMemoryView()
                    .environmentObject(appManager)
            case .settingsAppearance:
                SettingsAppearanceView()
                    .environmentObject(appManager)
            case .settingsSecurity:
                SettingsSecurityView()
                    .environmentObject(appManager)
            case .settingsTools:
                SettingsToolsView()
                    .environmentObject(appManager)
            case .toolDiscovery, .contextvmToolDetail:
                SettingsToolsView()
                    .environmentObject(appManager)
            case .trustedProviders:
                SettingsProvidersView()
                    .environmentObject(appManager)
            case .agents:
                AgentSessionListView()
                    .environmentObject(appManager)
            case .chat:
                ChatView(
                    state: appManager.appState,
                    inputText: $chatInputText,
                    onSend: { forceRole in
                        let text = chatInputText.trimmingCharacters(in: .whitespacesAndNewlines)
                        guard !text.isEmpty else { return }
                        appManager.dispatch(.sendMessage(text: text, forceRole: forceRole))
                        chatInputText = ""
                    },
                    onStop: { appManager.dispatch(.stopGeneration) },
                    onRetry: { appManager.dispatch(.retryLastMessage) },
                    onEdit: { id, text in appManager.dispatch(.editMessage(messageId: id, newText: text)) },
                    onCopy: { text in UIPasteboard.general.string = text },
                    onAttach: { filename, content, sizeBytes in
                        appManager.dispatch(.attachFile(
                            filename: filename,
                            content: content,
                            sizeBytes: sizeBytes
                        ))
                    },
                    onClearAttachment: { appManager.dispatch(.clearAttachment) },
                    onSelectModel: { model in appManager.dispatch(.selectModel(modelId: model)) },
                    onUseHybridProfile: { profileId in
                        if let convId = appManager.appState.currentConversationId {
                            appManager.dispatch(.overrideConversationBackend(
                                conversationId: convId,
                                backendId: "hybrid:\(profileId)"
                            ))
                        }
                        appManager.dispatch(.setActiveHybridProfile(profileId: profileId))
                    },
                    onSetSystemPrompt: { prompt in appManager.dispatch(.setSystemPrompt(prompt: prompt)) },
                    onSetToolsEnabled: { enabled in
                        if let convId = appManager.appState.currentConversationId {
                            appManager.dispatch(.setConversationToolsEnabled(conversationId: convId, enabled: enabled))
                        }
                    },
                    onRenameConversation: { id, title in
                        appManager.dispatch(.renameConversation(id: id, title: title))
                    },
                    onBack: { appManager.dispatch(.popScreen) },
                    onAttachDocument: { docId in appManager.dispatch(.attachDocumentToConversation(documentId: docId)) },
                    onDetachDocument: { docId in appManager.dispatch(.detachDocumentFromConversation(documentId: docId)) }
                )
                .environmentObject(appManager)
            case .home:
                homeView
                }
            }
        }
        // D-10: Record when app backgrounds; check elapsed time on return to foreground.
        .onChange(of: scenePhase) { _, newPhase in
            switch newPhase {
            case .background:
                backgroundedAt = Date()
            case .active:
                if let bg = backgroundedAt {
                    let elapsed = Date().timeIntervalSince(bg)
                    let timeout = Double(appManager.appState.lockTimeoutSeconds)
                    // -1 = Never. 0 = Immediately (always lock). Any positive value: lock if exceeded.
                    if timeout >= 0 && elapsed >= timeout {
                        appManager.dispatch(.lockApp)
                    }
                }
                backgroundedAt = nil
                // Phase 32 D-22: foreground-resume sync for all directory sources.
                // Runs after the lock-gate check so a locked app does not sync.
                if !appManager.appState.directorySources.isEmpty {
                    DirectorySyncScheduler.syncAll(appManager: appManager)
                }
            case .inactive:
                // backgroundedAt intentionally not reset here (WR-04).
                // iOS guarantees the .background phase is always reached before .active
                // whenever the app is truly backgrounded, so the elapsed-time check in
                // .active is always measured from the correct .background timestamp.
                // Transient .inactive entries (phone call overlay, Control Centre, app
                // switcher) do not affect the lock timeout calculation.
                break
            @unknown default:
                break
            }
        }
    }

    private var homeView: some View {
        NavigationStack {
            List {
                ForEach(appManager.appState.conversations, id: \.id) { conv in
                    Button(conv.title) {
                        appManager.dispatch(.loadConversation(conversationId: conv.id))
                    }
                }
            }
            .navigationTitle("Mango")
            .toolbar {
                ToolbarItem(placement: .navigationBarTrailing) {
                    HStack(spacing: 12) {
                        Button("RAG") {
                            appManager.dispatch(.pushScreen(screen: .documents))
                        }
                        .font(.subheadline)
                        Button("Settings") {
                            appManager.dispatch(.pushScreen(screen: .settings))
                        }
                    }
                }
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("New") {
                        appManager.dispatch(.newConversation)
                    }
                }
            }
        }
    }
}
