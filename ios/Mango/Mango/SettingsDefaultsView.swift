import SwiftUI

struct SettingsDefaultsView: View {
    @EnvironmentObject var appManager: AppManager

    @State private var defaultModel: String = ""
    @State private var defaultInstructions: String = ""
    @State private var defaultInstructionsInitialized: Bool = false

    var appState: AppState { appManager.appState }

    var body: some View {
        NavigationStack {
            List {
                defaultsSection
            }
            .navigationTitle("Defaults")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Back") { appManager.dispatch(.popScreen) }
                }
            }
        }
    }

    // MARK: - Defaults

    private var defaultsSection: some View {
        Section("Defaults") {
            let allModels = Array(Set(appState.backends.flatMap { $0.models })).sorted()
            if allModels.isEmpty {
                Text("Enable a provider to select a default model.")
                    .font(.subheadline).foregroundStyle(.secondary)
            } else {
                Picker("Default Model", selection: $defaultModel) {
                    Text("None").tag("")
                    ForEach(allModels, id: \.self) { Text($0).tag($0) }
                }
                .onChange(of: defaultModel) { _, v in
                    guard !v.isEmpty else { return }
                    appManager.dispatch(.setDefaultModel(modelId: v))
                }
            }

            VStack(alignment: .leading, spacing: 6) {
                Text("Default Instructions")
                    .font(.subheadline).fontWeight(.medium)
                Text("Fallback system prompt for conversations without custom instructions.")
                    .font(.caption).foregroundStyle(.secondary)
                TextEditor(text: $defaultInstructions)
                    .frame(minHeight: 80, maxHeight: 160)
                    .font(.body)
                    .overlay(
                        RoundedRectangle(cornerRadius: 8)
                            .stroke(Color.secondary.opacity(0.3), lineWidth: 1)
                    )
                Button("Save") {
                    let trimmed = defaultInstructions.trimmingCharacters(in: .whitespacesAndNewlines)
                    appManager.dispatch(.setGlobalSystemPrompt(prompt: trimmed.isEmpty ? nil : trimmed))
                }
                .buttonStyle(.borderedProminent).controlSize(.small)
            }
            .padding(.vertical, 4)
            .onAppear {
                if !defaultInstructionsInitialized {
                    defaultInstructions = appState.globalSystemPrompt ?? ""
                    defaultInstructionsInitialized = true
                }
            }
        }
    }
}
