import SwiftUI

/// Root content view: routes to Settings, Chat, or Home based on router state.
struct ContentView: View {
    @EnvironmentObject var appManager: AppManager
    @Environment(\.scenePhase) var scenePhase

    /// Timestamp when the app last moved to background (D-10).
    @State private var backgroundedAt: Date? = nil

    var body: some View {
        let screen = appManager.appState.router.currentScreen
        Group {
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
            case .documents:
                DocumentLibraryView()
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
            case .agents:
                AgentSessionListView()
                    .environmentObject(appManager)
            case .chat(let conversationId):
                ChatView(
                    state: appManager.appState,
                    inputText: .constant(""),
                    onSend: {},
                    onStop: { appManager.dispatch(.stopGeneration) },
                    onRetry: { appManager.dispatch(.retryLastMessage) },
                    onEdit: { id, text in appManager.dispatch(.editMessage(messageId: id, newText: text)) },
                    onCopy: { _ in },
                    onAttach: {},
                    onClearAttachment: { appManager.dispatch(.clearAttachment) },
                    onSelectModel: { model in appManager.dispatch(.selectModel(modelId: model)) },
                    onSetSystemPrompt: { prompt in appManager.dispatch(.setSystemPrompt(prompt: prompt)) },
                    onSetToolsEnabled: { enabled in
                        if let convId = appManager.appState.currentConversationId {
                            appManager.dispatch(.setConversationToolsEnabled(conversationId: convId, enabled: enabled))
                        }
                    },
                    onBack: { appManager.dispatch(.popScreen) },
                    onAttachDocument: { docId in appManager.dispatch(.attachDocumentToConversation(documentId: docId)) },
                    onDetachDocument: { docId in appManager.dispatch(.detachDocumentFromConversation(documentId: docId)) }
                )
            case .home:
                homeView
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
            default:
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
                        Button("Documents") {
                            appManager.dispatch(.pushScreen(screen: .documents))
                        }
                        .font(.subheadline)
                        Button("Agents") {
                            appManager.dispatch(.pushScreen(screen: .agents))
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
