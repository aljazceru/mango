import SwiftUI

struct SettingsMemoryView: View {
    @EnvironmentObject var appManager: AppManager

    var appState: AppState { appManager.appState }

    var body: some View {
        NavigationStack {
            List {
                Section("Memory") {
                    Toggle(isOn: Binding(
                        get: { appState.memoriesEnabled },
                        set: { appManager.dispatch(.setMemoriesEnabled(enabled: $0)) }
                    )) {
                        VStack(alignment: .leading, spacing: 4) {
                            Text("Auto-extract memories")
                            Text("Extract memories after each conversation and store them locally.")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }

                    Button {
                        appManager.dispatch(.pushScreen(screen: .memories))
                    } label: {
                        HStack {
                            VStack(alignment: .leading, spacing: 2) {
                                Text("Manage memories")
                                    .foregroundStyle(.primary)
                                Text(appState.memoryCount > 0 ? "\(appState.memoryCount) saved" : "No saved memories")
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                            Spacer()
                            Image(systemName: "chevron.right")
                                .font(.caption)
                                .foregroundStyle(.tertiary)
                        }
                    }
                }
            }
            .navigationTitle("Memory")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Back") { appManager.dispatch(.popScreen) }
                }
            }
        }
    }
}
