import SwiftUI

/// Root content view: routes to Settings, Chat, or Home based on router state.
struct ContentView: View {
    @EnvironmentObject var appManager: AppManager

    var body: some View {
        let screen = appManager.appState.router.currentScreen
        switch screen {
        case .onboarding(let step):
            OnboardingView(step: step)
                .environmentObject(appManager)
        case .settings:
            SettingsView()
                .environmentObject(appManager)
        case .documents:
            DocumentLibraryView()
                .environmentObject(appManager)
        // .agents — hidden until polished
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
                onBack: { appManager.dispatch(.popScreen) },
                onAttachDocument: { docId in appManager.dispatch(.attachDocumentToConversation(documentId: docId)) },
                onDetachDocument: { docId in appManager.dispatch(.detachDocumentFromConversation(documentId: docId)) }
            )
        case .home:
            homeView
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
