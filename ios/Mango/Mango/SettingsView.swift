import SwiftUI

struct SettingsView: View {
    @EnvironmentObject var appManager: AppManager
    @Environment(\.colorScheme) private var colorScheme

    @State private var presetKeys: [String: String] = [:]
    @State private var showAdvanced: Bool = false

    // Custom backend form (advanced)
    @State private var addName: String = ""
    @State private var addUrl: String = ""
    @State private var addApiKey: String = ""
    @State private var addTeeType: String = "IntelTdx"

    // Re-attestation interval (advanced)
    @State private var attestationIntervalInput: String = ""

    @State private var defaultModel: String = ""
    @State private var defaultInstructions: String = ""
    @State private var defaultInstructionsInitialized: Bool = false
    @AppStorage("theme_preference") private var themePreference: String = "system"

    var appState: AppState { appManager.appState }

    var body: some View {
        NavigationStack {
            List {
                providersSection
                defaultsSection
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
        Section {
            let presets = knownProviderPresets()
            ForEach(presets, id: \.id) { preset in
                let isEnabled = appState.backends.contains(where: { $0.id == preset.id && $0.hasApiKey })
                if isEnabled {
                    enabledRow(preset)
                } else {
                    disabledRow(preset)
                }
            }
        } header: {
            Text("Providers")
        }
    }

    @ViewBuilder
    private func enabledRow(_ preset: ProviderPreset) -> some View {
        let backend = appState.backends.first(where: { $0.id == preset.id })
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(preset.name).font(.body).fontWeight(.medium)
                    Text(teeTypeLabel(preset.teeType))
                        .font(.caption).foregroundStyle(.secondary)
                }
                Spacer()
                Text("Enabled")
                    .font(.caption2).fontWeight(.semibold)
                    .foregroundStyle(AppColors.healthSuccess(colorScheme))
                    .padding(.horizontal, 8).padding(.vertical, 3)
                    .background(AppColors.healthSuccess(colorScheme).opacity(0.12))
                    .clipShape(Capsule())
            }

            if let backend = backend {
                HStack(spacing: 6) {
                    Text(healthLabel(backend.healthStatus))
                        .font(.caption2).fontWeight(.medium)
                        .foregroundStyle(healthColor(backend.healthStatus, colorScheme))
                        .padding(.horizontal, 6).padding(.vertical, 2)
                        .background(healthColor(backend.healthStatus, colorScheme).opacity(0.10))
                        .clipShape(Capsule())

                    if let att = appState.attestationStatuses.first(where: { $0.backendId == backend.id }) {
                        let (label, color) = attestationStyle(att.status, colorScheme)
                        Text(label)
                            .font(.caption2)
                            .foregroundStyle(color)
                    }
                }

                if !backend.models.isEmpty {
                    Text(backend.models.prefix(3).joined(separator: " · "))
                        .font(.caption2).foregroundStyle(.tertiary)
                        .lineLimit(1)
                }
            }

            HStack(spacing: 8) {
                if let backend = backend {
                    if backend.isActive {
                        Label("Default", systemImage: "checkmark.seal.fill")
                            .font(.caption2).fontWeight(.medium)
                            .foregroundStyle(AppColors.healthSuccess(colorScheme))
                    } else {
                        Button("Set Default") {
                            appManager.dispatch(.setDefaultBackend(backendId: preset.id))
                        }
                        .buttonStyle(.bordered).controlSize(.mini)
                    }
                }
                Spacer()
                Button("Remove") {
                    appManager.dispatch(.removeBackend(backendId: preset.id))
                }
                .buttonStyle(.bordered).controlSize(.mini).tint(.red)
            }
        }
        .padding(.vertical, 6)
    }

    @ViewBuilder
    private func disabledRow(_ preset: ProviderPreset) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            VStack(alignment: .leading, spacing: 2) {
                Text(preset.name).font(.body).fontWeight(.medium)
                Text(preset.description).font(.caption).foregroundStyle(.secondary)
                Text(teeTypeLabel(preset.teeType)).font(.caption2).foregroundStyle(.tertiary)
            }
            SecureField("API Key", text: Binding(
                get: { presetKeys[preset.id] ?? "" },
                set: { presetKeys[preset.id] = $0 }
            ))
            .textFieldStyle(.roundedBorder)
            .autocorrectionDisabled()
            .textInputAutocapitalization(.never)

            Button("Enable") {
                let key = (presetKeys[preset.id] ?? "").trimmingCharacters(in: .whitespaces)
                guard !key.isEmpty else { return }
                appManager.dispatch(.addBackendFromPreset(presetId: preset.id, apiKey: key))
                presetKeys[preset.id] = ""
            }
            .buttonStyle(.borderedProminent).controlSize(.small)
            .tint(AppColors.healthSuccess(colorScheme))
            .disabled((presetKeys[preset.id] ?? "").trimmingCharacters(in: .whitespaces).isEmpty)
        }
        .padding(.vertical, 4)
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

    private func healthLabel(_ s: HealthStatus) -> String {
        switch s {
        case .healthy: return "Healthy"
        case .degraded: return "Degraded"
        case .failed: return "Failed"
        case .unknown: return "Unknown"
        }
    }

    private func healthColor(_ s: HealthStatus, _ scheme: ColorScheme) -> Color {
        switch s {
        case .healthy:  return AppColors.healthSuccess(scheme)
        case .degraded: return AppColors.healthWarning(scheme)
        case .failed:   return AppColors.destructive(scheme)
        case .unknown:  return AppColors.healthMuted(scheme)
        }
    }

    private func attestationStyle(_ s: AttestationStatus, _ scheme: ColorScheme) -> (String, Color) {
        switch s {
        case .verified:   return ("Attested",       AppColors.healthSuccess(scheme))
        case .unverified: return ("Unverified",     AppColors.healthMuted(scheme))
        case .failed:     return ("Attest Failed",  AppColors.destructive(scheme))
        case .expired:    return ("Attest Expired", AppColors.healthWarning(scheme))
        }
    }

    private func teeTypeLabel(_ t: TeeType) -> String {
        switch t {
        case .intelTdx:     return "Intel TDX"
        case .nvidiaH100Cc: return "NVIDIA H100 CC"
        case .amdSevSnp:    return "AMD SEV-SNP"
        case .unknown:      return "Unknown"
        }
    }

    private func parseTeeType(_ s: String) -> TeeType {
        switch s {
        case "NvidiaH100Cc": return .nvidiaH100Cc
        case "AmdSevSnp":    return .amdSevSnp
        case "Unknown":      return .unknown
        default:             return .intelTdx
        }
    }
}
