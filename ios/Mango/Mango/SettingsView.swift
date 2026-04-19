import SwiftUI

struct SettingsView: View {
    @EnvironmentObject var appManager: AppManager
    @AppStorage("theme_preference") private var themePreference: String = "system"

    var appState: AppState { appManager.appState }

    var body: some View {
        NavigationStack {
            List {
                providersSection
                defaultsSection
                directorySourcesSection
                memorySection
                securitySection
                toolsSection
                appearanceSection
            }
            .navigationTitle("Settings")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Back") { appManager.dispatch(.popScreen) }
                }
            }
        }
    }

    private var providersSection: some View {
        Section("Providers") {
            settingsRow(
                title: "Providers",
                detail: "\(appState.backends.filter { $0.hasApiKey }.count) enabled"
            ) {
                appManager.dispatch(.pushScreen(screen: .settingsProviders))
            }
        }
    }

    private var defaultsSection: some View {
        Section("Defaults") {
            settingsRow(
                title: "Defaults",
                detail: appState.backends.first(where: { $0.isActive })?.models.first
            ) {
                appManager.dispatch(.pushScreen(screen: .settingsDefaults))
            }
        }
    }

    private var directorySourcesSection: some View {
        Section("Directory Sources") {
            settingsRow(
                title: "Directory Sources",
                detail: directorySourcesSummary
            ) {
                appManager.dispatch(.pushScreen(screen: .directorySources))
            }
        }
    }

    private var directorySourcesSummary: String {
        let n = appState.directorySources.count
        if n == 0 { return "No folders added" }
        return n == 1 ? "1 folder" : "\(n) folders"
    }

    private var memorySection: some View {
        Section("Memory") {
            settingsRow(
                title: "Memory",
                detail: memorySummary
            ) {
                appManager.dispatch(.pushScreen(screen: .settingsMemory))
            }
        }
    }

    private var securitySection: some View {
        Section("Security") {
            settingsRow(
                title: "Security",
                detail: securitySummary
            ) {
                appManager.dispatch(.pushScreen(screen: .settingsSecurity))
            }
        }
    }

    private var toolsSection: some View {
        Section("Tools") {
            settingsRow(
                title: "Tools",
                detail: appState.braveApiKeySet ? "Web search configured" : "Web search not configured"
            ) {
                appManager.dispatch(.pushScreen(screen: .settingsTools))
            }
        }
    }

    private var appearanceSection: some View {
        Section("Appearance") {
            settingsRow(
                title: "Appearance",
                detail: appearanceSummary
            ) {
                appManager.dispatch(.pushScreen(screen: .settingsAppearance))
            }
        }
    }

    private var memorySummary: String {
        var summary = appState.memoriesEnabled ? "Auto-extract on" : "Auto-extract off"
        if appState.memoryCount > 0 {
            summary += " • \(appState.memoryCount)"
        }
        return summary
    }

    private var securitySummary: String {
        var parts = [lockTimeoutLabel(appState.lockTimeoutSeconds)]
        if appState.duressPinConfigured {
            parts.append("Duress PIN set")
        }
        parts.append(appState.biometricLoginEnabled ? "Biometrics on" : "Biometrics off")
        return parts.joined(separator: " • ")
    }

    private var appearanceSummary: String {
        switch themePreference {
        case "light": return "Force Light"
        case "dark": return "Force Dark"
        default: return "Follow System"
        }
    }

    private func settingsRow(title: String, detail: String?, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text(title)
                        .font(.body)
                        .fontWeight(.medium)
                        .foregroundStyle(.primary)
                    if let detail, !detail.isEmpty {
                        Text(detail)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                    }
                }
                Spacer()
                Image(systemName: "chevron.right")
                    .font(.caption)
                    .foregroundStyle(.tertiary)
            }
        }
    }
}

func lockTimeoutLabel(_ seconds: Int64) -> String {
    switch seconds {
    case 0: return "Immediately"
    case 60: return "1 minute"
    case 300: return "5 minutes"
    case 900: return "15 minutes"
    case -1: return "Never"
    default: return "5 minutes"
    }
}
