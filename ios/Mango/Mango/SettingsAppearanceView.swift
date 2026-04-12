import SwiftUI

struct SettingsAppearanceView: View {
    @AppStorage("theme_preference") private var themePreference: String = "system"
    @EnvironmentObject var appManager: AppManager

    var body: some View {
        NavigationStack {
            List {
                Section("Appearance") {
                    appearanceRow("Follow System", value: "system", detail: "Match the device appearance.")
                    appearanceRow("Force Light", value: "light", detail: nil)
                    appearanceRow("Force Dark", value: "dark", detail: nil)
                }
            }
            .navigationTitle("Appearance")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Back") { appManager.dispatch(.popScreen) }
                }
            }
        }
    }

    private func appearanceRow(_ title: String, value: String, detail: String?) -> some View {
        Button {
            themePreference = value
        } label: {
            HStack(spacing: 12) {
                Image(systemName: themePreference == value ? "largecircle.fill.circle" : "circle")
                    .foregroundStyle(.tint)
                VStack(alignment: .leading, spacing: 2) {
                    Text(title).foregroundStyle(.primary)
                    if let detail {
                        Text(detail)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
                Spacer()
            }
        }
    }
}
