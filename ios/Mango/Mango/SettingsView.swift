import SwiftUI

struct SettingsView: View {
    @EnvironmentObject var appManager: AppManager

    @State private var showAdvanced: Bool = false

    // Custom backend form (advanced)
    @State private var addName: String = ""
    @State private var addUrl: String = ""
    @State private var addApiKey: String = ""
    @State private var addTeeType: String = "IntelTdx"

    // Re-attestation interval (advanced)
    @State private var attestationIntervalInput: String = ""

    @State private var braveApiKeyInput: String = ""
    @State private var braveApiKeyMessage: String? = nil
    @AppStorage("theme_preference") private var themePreference: String = "system"

    var appState: AppState { appManager.appState }

    var body: some View {
        NavigationStack {
            List {
                providersSection
                defaultsSection
                memorySection
                toolsSection
                appearanceSection
                advancedSection
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

    // MARK: - Providers

    private var providersSection: some View {
        Section("Providers") {
            Button(action: { appManager.dispatch(.pushScreen(screen: .settingsProviders)) }) {
                HStack {
                    Text("Providers")
                        .font(.body).fontWeight(.medium)
                        .foregroundStyle(.primary)
                    Spacer()
                    let enabledCount = appState.backends.filter { $0.hasApiKey }.count
                    if enabledCount > 0 {
                        Text("\(enabledCount) enabled")
                            .font(.caption).foregroundStyle(.secondary)
                    }
                    Image(systemName: "chevron.right")
                        .font(.caption).foregroundStyle(.tertiary)
                }
            }
        }
    }

    // MARK: - Defaults

    private var defaultsSection: some View {
        Section("Defaults") {
            Button(action: { appManager.dispatch(.pushScreen(screen: .settingsDefaults)) }) {
                HStack {
                    Text("Defaults")
                        .font(.body).fontWeight(.medium)
                        .foregroundStyle(.primary)
                    Spacer()
                    let activeModel = appState.backends.first(where: { $0.isActive })?.models.first
                    if let model = activeModel {
                        Text(model)
                            .font(.caption).foregroundStyle(.secondary)
                            .lineLimit(1)
                    }
                    Image(systemName: "chevron.right")
                        .font(.caption).foregroundStyle(.tertiary)
                }
            }
        }
    }

    // MARK: - Memory

    private var memorySection: some View {
        Section("Memory") {
            Toggle(isOn: Binding(
                get: { appState.memoriesEnabled },
                set: { appManager.dispatch(.setMemoriesEnabled(enabled: $0)) }
            )) {
                Label("Auto-extract Memories", systemImage: "brain")
            }
            Button(action: { appManager.dispatch(.pushScreen(screen: .memories)) }) {
                HStack {
                    Label("Memories", systemImage: "brain")
                        .foregroundStyle(.primary)
                    Spacer()
                    if appState.memoryCount > 0 {
                        Text("\(appState.memoryCount)")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    Image(systemName: "chevron.right")
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                }
            }
        }
    }

    // MARK: - Tools

    private var toolsSection: some View {
        Section("Tools") {
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Text("Web Search")
                        .font(.subheadline).fontWeight(.medium)
                    Spacer()
                    if appState.braveApiKeyValidating {
                        ProgressView().scaleEffect(0.7)
                    } else if appState.braveApiKeySet {
                        HStack(spacing: 4) {
                            Image(systemName: "checkmark.circle.fill")
                                .font(.caption)
                                .foregroundStyle(.green)
                            Text("Configured")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
                Text("Required for agent web search. Keys are stored locally and never sent to third parties.")
                    .font(.caption).foregroundStyle(.secondary)
                SecureField(
                    appState.braveApiKeySet
                        ? "Key configured — enter new key to update"
                        : "Enter Brave Search API Key",
                    text: $braveApiKeyInput
                )
                .textFieldStyle(.roundedBorder)
                .disabled(appState.braveApiKeyValidating)
                if let msg = braveApiKeyMessage {
                    Text(msg)
                        .font(.caption)
                        .foregroundStyle(msg.contains("saved") ? Color.green : Color.red)
                }
                Button(appState.braveApiKeyValidating ? "Verifying…" : "Save API Key") {
                    let trimmed = braveApiKeyInput.trimmingCharacters(in: .whitespacesAndNewlines)
                    if !trimmed.isEmpty {
                        braveApiKeyMessage = nil
                        appManager.dispatch(.validateBraveApiKey(apiKey: trimmed))
                        braveApiKeyInput = ""
                    }
                }
                .buttonStyle(.borderedProminent).controlSize(.small)
                .disabled(
                    braveApiKeyInput.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                    || appState.braveApiKeyValidating
                )
            }
            .padding(.vertical, 4)
            .onChange(of: appState.toast) { _, newToast in
                // Mirror the Rust-side toast into our local inline message when it
                // relates to Brave key validation (the only source of toasts on this screen).
                if let t = newToast {
                    braveApiKeyMessage = t
                    appManager.dispatch(.clearToast)
                }
            }
        }
    }

    // MARK: - Appearance

    private var appearanceSection: some View {
        Section("Appearance") {
            Picker("Appearance", selection: $themePreference) {
                Text("Follow System").tag("system")
                Text("Force Light").tag("light")
                Text("Force Dark").tag("dark")
            }
            .pickerStyle(.menu)
        }
    }

    // MARK: - Advanced Settings

    private var advancedSection: some View {
        Section {
            DisclosureGroup(isExpanded: $showAdvanced) {
                VStack(alignment: .leading, spacing: 20) {

                    // Re-attestation Interval
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Re-attestation Interval")
                            .font(.subheadline).fontWeight(.medium)
                        Text("How often the active provider is automatically re-attested. 0 = disabled.")
                            .font(.caption).foregroundStyle(.secondary)

                        let current = appState.attestationIntervalMinutes
                        Stepper(
                            "Every \(attestationIntervalInput.isEmpty ? "\(current)" : attestationIntervalInput) min",
                            onIncrement: {
                                let base = Int(attestationIntervalInput) ?? Int(current)
                                let next = max(0, base + 1)
                                attestationIntervalInput = "\(next)"
                                appManager.dispatch(.setAttestationInterval(minutes: UInt32(next)))
                            },
                            onDecrement: {
                                let base = Int(attestationIntervalInput) ?? Int(current)
                                let next = max(0, base - 1)
                                attestationIntervalInput = "\(next)"
                                appManager.dispatch(.setAttestationInterval(minutes: UInt32(next)))
                            }
                        )
                    }

                    Divider()

                    // Custom Provider
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Custom Provider")
                            .font(.subheadline).fontWeight(.medium)
                        Text("For self-hosted or experimental confidential inference endpoints.")
                            .font(.caption).foregroundStyle(.secondary)

                        TextField("Name", text: $addName).autocorrectionDisabled()
                        TextField("Base URL", text: $addUrl)
                            .keyboardType(.URL).autocorrectionDisabled()
                            .textInputAutocapitalization(.never)
                        SecureField("API Key", text: $addApiKey)
                        Picker("TEE Type", selection: $addTeeType) {
                            Text("Intel TDX").tag("IntelTdx")
                            Text("NVIDIA H100 CC").tag("NvidiaH100Cc")
                            Text("AMD SEV-SNP").tag("AmdSevSnp")
                            Text("Unknown").tag("Unknown")
                        }
                        Button("Add Provider") {
                            appManager.dispatch(.addBackend(
                                name: addName, baseUrl: addUrl, apiKey: addApiKey,
                                teeType: parseTeeType(addTeeType), models: []
                            ))
                            addName = ""; addUrl = ""; addApiKey = ""; addTeeType = "IntelTdx"
                        }
                        .buttonStyle(.borderedProminent).controlSize(.small)
                        .disabled(
                            addName.trimmingCharacters(in: .whitespaces).isEmpty
                            || addUrl.trimmingCharacters(in: .whitespaces).isEmpty
                            || addApiKey.isEmpty
                        )
                    }
                }
                .padding(.top, 8)
            } label: {
                Label("Advanced Settings", systemImage: "gearshape.2")
                    .font(.subheadline).fontWeight(.medium)
            }
        }
    }

    // MARK: - Helpers

    private func parseTeeType(_ s: String) -> TeeType {
        switch s {
        case "NvidiaH100Cc": return .nvidiaH100Cc
        case "AmdSevSnp":    return .amdSevSnp
        case "Unknown":      return .unknown
        default:             return .intelTdx
        }
    }
}
