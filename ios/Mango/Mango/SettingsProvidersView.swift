import SwiftUI

struct SettingsProvidersView: View {
    @EnvironmentObject var appManager: AppManager
    @Environment(\.colorScheme) private var colorScheme

    @State private var presetKeys: [String: String] = [:]
    @State private var addName: String = ""
    @State private var addUrl: String = ""
    @State private var addApiKey: String = ""
    @State private var addTeeType: String = "IntelTdx"
    @State private var attestationIntervalInput: String = ""

    var appState: AppState { appManager.appState }

    var body: some View {
        NavigationStack {
            List {
                providersSection
                providerDefaultsSection
                customProviderSection
            }
            .navigationTitle("Providers")
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
            let presets = knownProviderPresets()
            ForEach(presets, id: \.id) { preset in
                let isEnabled = appState.backends.contains(where: { $0.id == preset.id && $0.hasApiKey })
                if isEnabled {
                    enabledRow(preset)
                } else {
                    disabledRow(preset)
                }
            }
        }
    }

    private var providerDefaultsSection: some View {
        Section("Provider Defaults") {
            VStack(alignment: .leading, spacing: 8) {
                Text("Re-attestation Interval")
                    .font(.subheadline)
                    .fontWeight(.medium)
                Text("How often the active provider is automatically re-attested. Set 0 to disable.")
                    .font(.caption)
                    .foregroundStyle(.secondary)

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
            .padding(.vertical, 4)
        }
    }

    private var customProviderSection: some View {
        Section("Custom Provider") {
            VStack(alignment: .leading, spacing: 8) {
                Text("For self-hosted or experimental confidential inference endpoints.")
                    .font(.caption)
                    .foregroundStyle(.secondary)

                TextField("Name", text: $addName)
                    .autocorrectionDisabled()
                TextField("Base URL", text: $addUrl)
                    .keyboardType(.URL)
                    .autocorrectionDisabled()
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
                        name: addName,
                        baseUrl: addUrl,
                        apiKey: addApiKey,
                        teeType: parseTeeType(addTeeType),
                        models: []
                    ))
                    addName = ""
                    addUrl = ""
                    addApiKey = ""
                    addTeeType = "IntelTdx"
                }
                .buttonStyle(.borderedProminent)
                .disabled(
                    addName.trimmingCharacters(in: .whitespaces).isEmpty
                    || addUrl.trimmingCharacters(in: .whitespaces).isEmpty
                    || addApiKey.isEmpty
                )
            }
            .padding(.vertical, 4)
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

            if let backend {
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

                // Phase 34.1: trust-UI sub-lines for Redpill freshness + orchestrated breakdown.
                // Copy LOCKED in 34.1-UI-SPEC.md.
                if let att = appState.attestationStatuses.first(where: { $0.backendId == backend.id }) {
                    if case let .verified(_, freshness, components) = att.status {
                        if freshness == "PerEnclave" {
                            Text("Verified for this enclave instance")
                                .font(.caption2)
                                .foregroundColor(.secondary)
                        }
                        if let comps = components, !comps.isEmpty {
                            let labelMap: [String: String] = [
                                "gateway": "gateway",
                                "model": "model",
                                "compose_manager": "compose",
                            ]
                            let line = comps
                                .map { "\(labelMap[$0.label] ?? $0.label) ✓" }
                                .joined(separator: " • ")
                            Text(line)
                                .font(.caption2)
                                .foregroundColor(.secondary)
                        }
                    }
                }

                if !backend.models.isEmpty {
                    Text(backend.models.prefix(3).joined(separator: " · "))
                        .font(.caption2)
                        .foregroundStyle(.tertiary)
                        .lineLimit(1)
                }

                HStack(spacing: 8) {
                    if backend.isActive {
                        Label("Default", systemImage: "checkmark.seal.fill")
                            .font(.caption2)
                            .fontWeight(.medium)
                            .foregroundStyle(AppColors.healthSuccess(colorScheme))
                    } else {
                        Button("Set Default") {
                            appManager.dispatch(.setDefaultBackend(backendId: preset.id))
                        }
                        .buttonStyle(.bordered)
                        .controlSize(.mini)
                    }
                    Spacer()
                    Button("Remove") {
                        appManager.dispatch(.removeBackend(backendId: preset.id))
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.mini)
                    .tint(.red)
                }
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
            .buttonStyle(.borderedProminent)
            .controlSize(.small)
            .tint(AppColors.healthSuccess(colorScheme))
            .disabled((presetKeys[preset.id] ?? "").trimmingCharacters(in: .whitespaces).isEmpty)
        }
        .padding(.vertical, 4)
    }

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
        case .verified(_, _, _): return ("Attested",       AppColors.healthSuccess(scheme))
        case .unverified:        return ("Unverified",     AppColors.healthMuted(scheme))
        case .failed:            return ("Attest Failed",  AppColors.destructive(scheme))
        case .expired:           return ("Attest Expired", AppColors.healthWarning(scheme))
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
        case "AmdSevSnp": return .amdSevSnp
        case "Unknown": return .unknown
        default: return .intelTdx
        }
    }
}
