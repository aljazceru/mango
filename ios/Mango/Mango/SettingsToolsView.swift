import SwiftUI

struct SettingsToolsView: View {
    @EnvironmentObject var appManager: AppManager

    @State private var braveApiKeyInput: String = ""
    @State private var braveApiKeyMessage: String? = nil

    var appState: AppState { appManager.appState }

    var body: some View {
        NavigationStack {
            List {
                Section("Tools") {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Web Search")
                            .font(.subheadline)
                            .fontWeight(.medium)

                        statusRow

                        Text("Required for agent web search. Keys stay on-device until used for Brave requests.")
                            .font(.caption)
                            .foregroundStyle(.secondary)

                        SecureField(
                            appState.braveApiKeySet
                                ? "Key configured — enter new key to update"
                                : "Enter Brave Search API Key",
                            text: $braveApiKeyInput
                        )
                        .textFieldStyle(.roundedBorder)
                        .disabled(appState.braveApiKeyValidating)

                        if let braveApiKeyMessage {
                            Text(braveApiKeyMessage)
                                .font(.caption)
                                .foregroundStyle(braveApiKeyMessage.localizedCaseInsensitiveContains("saved") ? .green : .red)
                        }

                        Button(appState.braveApiKeyValidating ? "Verifying…" : "Save API Key") {
                            let trimmed = braveApiKeyInput.trimmingCharacters(in: .whitespacesAndNewlines)
                            guard !trimmed.isEmpty else { return }
                            braveApiKeyMessage = nil
                            appManager.dispatch(.validateBraveApiKey(apiKey: trimmed))
                            braveApiKeyInput = ""
                        }
                        .buttonStyle(.borderedProminent)
                        .disabled(
                            braveApiKeyInput.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                            || appState.braveApiKeyValidating
                        )
                    }
                    .padding(.vertical, 4)
                }
            }
            .navigationTitle("Tools")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Back") { appManager.dispatch(.popScreen) }
                }
            }
            .onChange(of: appState.toast) { _, toast in
                if let toast {
                    braveApiKeyMessage = toast
                    appManager.dispatch(.clearToast)
                }
            }
        }
    }

    @ViewBuilder
    private var statusRow: some View {
        if appState.braveApiKeyValidating {
            HStack(spacing: 6) {
                ProgressView()
                    .scaleEffect(0.8)
                Text("Verifying key")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        } else if appState.braveApiKeySet {
            HStack(spacing: 6) {
                Image(systemName: "checkmark.circle.fill")
                    .foregroundStyle(.green)
                Text("Configured")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        } else {
            Text("Not configured")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
    }
}
