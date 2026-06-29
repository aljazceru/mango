import Foundation
import SwiftUI

struct SettingsView: View {
    @EnvironmentObject var appManager: AppManager
    @AppStorage("theme_preference") private var themePreference: String = "system"
    @State private var selectedHybridLocalBackendId: String = ""
    @State private var selectedHybridLocalModelId: String = ""
    @State private var selectedHybridRemoteBackendId: String = ""
    @State private var selectedHybridRemoteModelId: String = ""

    var appState: AppState { appManager.appState }

    var body: some View {
        NavigationStack {
            List {
                providersSection
                defaultsSection
                if appState.localDeviceCapability.maxModelBytes > 0 {
                    localModelsSection
                }
                hybridRoutingSection
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
                detail: "\(appState.backends.filter { $0.hasApiKey || $0.id == "qvac-local" }.count) enabled"
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

    private var localModelsSection: some View {
        Section("On-device Models") {
            VStack(alignment: .leading, spacing: 8) {
                Toggle("On-device inference", isOn: Binding(
                    get: { appState.localInferenceEnabled },
                    set: { enabled in
                        appManager.dispatch(.setLocalInferenceEnabled(enabled: enabled))
                    }
                ))
                Text(localCapabilitySummary)
                    .font(.caption)
                    .foregroundStyle(.secondary)

                if let progress = appState.localDownloadProgress {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("\(localProgressLabel(progress.stage)): \(localProgressBytes(progress.downloadedBytes, progress.totalBytes))")
                            .font(.caption)
                            .foregroundStyle(.tint)
                        ProgressView()
                    }
                }

                if appState.localModels.isEmpty {
                    Text("No local models are available for this build.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
            .padding(.vertical, 4)

            ForEach(appState.localModels, id: \.id) { model in
                localModelRow(model)
            }
        }
    }

    @ViewBuilder
    private func localModelRow(_ model: LocalModelSummary) -> some View {
        let capability = appState.localDeviceCapability
        let anyDownloadActive = appState.localDownloadProgress != nil
        let busy = appState.localDownloadProgress?.modelId == model.id
        let supported = capability.maxModelBytes >= model.sizeBytes
            && capability.maxModelBytes > 0
            && capability.totalRamBytes >= model.minRamBytes
        let installed = model.downloaded && model.verified
        let canDownload = !installed && supported && !anyDownloadActive

        VStack(alignment: .leading, spacing: 8) {
            Text(model.name)
                .font(.subheadline)
                .fontWeight(.medium)
            Text([
                model.quantization,
                localBytes(model.sizeBytes),
                installed ? "Installed" : supported ? "Not installed" : "Unsupported"
            ].joined(separator: " • "))
                .font(.caption)
                .foregroundStyle(.secondary)
            HStack {
                if installed {
                    Button("Delete") {
                        appManager.dispatch(.deleteLocalModel(modelId: model.id))
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                    .disabled(anyDownloadActive)
                } else {
                    Button(busy ? "Downloading" : "Download") {
                        appManager.dispatch(.downloadLocalModel(modelId: model.id))
                    }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.small)
                    .disabled(!canDownload)
                }
            }
        }
        .padding(.vertical, 4)
    }

    private var hybridRoutingSection: some View {
        Section("Hybrid Routing") {
            let localBackends = appState.backends.filter { isHybridLocalBackend($0) }
            let remoteBackends = appState.backends.filter { isHybridRemoteBackend($0) }
            let existing = appState.hybridProfiles.first
            let localBackend = selectedBackend(
                from: localBackends,
                selectedId: selectedHybridLocalBackendId,
                existingId: existing?.localBackendId
            )
            let remoteBackend = selectedBackend(
                from: remoteBackends,
                selectedId: selectedHybridRemoteBackendId,
                existingId: existing?.remoteBackendId
            )

            VStack(alignment: .leading, spacing: 8) {
                Text("Local to confidential")
                    .font(.subheadline)
                    .fontWeight(.medium)
                Text("Use local inference by default and escalate selected turns to a confidential provider.")
                    .font(.caption)
                    .foregroundStyle(.secondary)

                if let localBackend, let remoteBackend {
                    let localModel = selectedModel(
                        from: localBackend,
                        selectedId: selectedHybridLocalModelId,
                        existingId: existing?.localModelId
                    )
                    let remoteModel = selectedModel(
                        from: remoteBackend,
                        selectedId: selectedHybridRemoteModelId,
                        existingId: existing?.remoteModelId
                    )
                    let profile = defaultHybridProfile(
                        localBackend: localBackend,
                        localModelId: localModel,
                        remoteBackend: remoteBackend,
                        remoteModelId: remoteModel,
                        existingProfile: existing
                    )
                    Text("\(localBackend.name) / \(compactModelName(profile.localModelId)) -> \(remoteBackend.name) / \(compactModelName(profile.remoteModelId))")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                    Picker("Local backend", selection: Binding(
                        get: { localBackend.id },
                        set: { backendId in
                            selectedHybridLocalBackendId = backendId
                            selectedHybridLocalModelId = localBackends
                                .first(where: { $0.id == backendId })?
                                .models
                                .first ?? ""
                        }
                    )) {
                        ForEach(localBackends, id: \.id) { backend in
                            Text(backend.name).tag(backend.id)
                        }
                    }
                    Picker("Local model", selection: Binding(
                        get: { localModel },
                        set: { selectedHybridLocalModelId = $0 }
                    )) {
                        ForEach(localBackend.models, id: \.self) { modelId in
                            Text(compactModelName(modelId)).tag(modelId)
                        }
                    }
                    Picker("Remote backend", selection: Binding(
                        get: { remoteBackend.id },
                        set: { backendId in
                            selectedHybridRemoteBackendId = backendId
                            selectedHybridRemoteModelId = remoteBackends
                                .first(where: { $0.id == backendId })?
                                .models
                                .first ?? ""
                        }
                    )) {
                        ForEach(remoteBackends, id: \.id) { backend in
                            Text(backend.name).tag(backend.id)
                        }
                    }
                    Picker("Remote model", selection: Binding(
                        get: { remoteModel },
                        set: { selectedHybridRemoteModelId = $0 }
                    )) {
                        ForEach(remoteBackend.models, id: \.self) { modelId in
                            Text(compactModelName(modelId)).tag(modelId)
                        }
                    }
                    Button(existing == nil ? "Create Profile" : "Update Profile") {
                        appManager.dispatch(.saveHybridProfile(profile: profile))
                    }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.small)
                } else if localBackends.isEmpty {
                    Text("No local-capable backend is available in this build. On-device models appear here when the iOS runtime exposes them.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                } else {
                    Text("Enable a healthy confidential provider with at least one model to pair with local routing.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
            .padding(.vertical, 4)

            ForEach(appState.hybridProfiles, id: \.id) { profile in
                hybridProfileRow(profile)
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

    @ViewBuilder
    private func hybridProfileRow(_ profile: HybridProfile) -> some View {
        let activeId = appState.activeBackendId?.hasPrefix("hybrid:") == true
            ? String(appState.activeBackendId!.dropFirst("hybrid:".count))
            : nil
        let isActive = activeId == profile.id

        VStack(alignment: .leading, spacing: 8) {
            Text(profile.name)
                .font(.subheadline)
                .fontWeight(.medium)
            Text("\(compactModelName(profile.localModelId)) -> \(compactModelName(profile.remoteModelId))")
                .font(.caption)
                .foregroundStyle(.secondary)

            HStack {
                Button(isActive ? "Default" : "Use by default") {
                    appManager.dispatch(.setActiveHybridProfile(profileId: profile.id))
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .disabled(isActive)

                Button("Delete") {
                    appManager.dispatch(.deleteHybridProfile(profileId: profile.id))
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .tint(.red)
            }

            Toggle("Attachments remote", isOn: Binding(
                get: { profile.policy.escalateIfAttachment },
                set: { value in
                    var updated = profile
                    updated.policy.escalateIfAttachment = value
                    appManager.dispatch(.saveHybridProfile(profile: updated))
                }
            ))
            Toggle("Offline local", isOn: Binding(
                get: { profile.policy.preferLocalWhenOffline },
                set: { value in
                    var updated = profile
                    updated.policy.preferLocalWhenOffline = value
                    appManager.dispatch(.saveHybridProfile(profile: updated))
                }
            ))
            Toggle("Long prompts remote", isOn: Binding(
                get: { profile.policy.escalateIfMessageLongerThan != nil },
                set: { value in
                    var updated = profile
                    updated.policy.escalateIfMessageLongerThan = value ? 4000 : nil
                    appManager.dispatch(.saveHybridProfile(profile: updated))
                }
            ))
        }
        .padding(.vertical, 4)
    }

    private func isHybridLocalBackend(_ backend: BackendSummary) -> Bool {
        (backend.id.hasPrefix("local-") || backend.id == "qvac-local") && !backend.models.isEmpty
    }

    private func isHybridRemoteBackend(_ backend: BackendSummary) -> Bool {
        !isHybridLocalBackend(backend)
            && backend.teeType != .unknown
            && backend.hasApiKey
            && !backend.models.isEmpty
            && backend.healthStatus != .failed
    }

    private func selectedBackend(
        from backends: [BackendSummary],
        selectedId: String,
        existingId: String?
    ) -> BackendSummary? {
        backends.first(where: { $0.id == selectedId })
            ?? existingId.flatMap { id in backends.first(where: { $0.id == id }) }
            ?? backends.first
    }

    private func selectedModel(
        from backend: BackendSummary,
        selectedId: String,
        existingId: String?
    ) -> String {
        if backend.models.contains(selectedId) {
            return selectedId
        }
        if let existingId, backend.models.contains(existingId) {
            return existingId
        }
        return backend.models.first ?? ""
    }

    private func defaultHybridProfile(
        localBackend: BackendSummary,
        localModelId: String,
        remoteBackend: BackendSummary,
        remoteModelId: String,
        existingProfile: HybridProfile?
    ) -> HybridProfile {
        HybridProfile(
            id: existingProfile?.id ?? "default_hybrid",
            name: "\(localBackend.name) -> \(remoteBackend.name)",
            localBackendId: localBackend.id,
            localModelId: localModelId,
            remoteBackendId: remoteBackend.id,
            remoteModelId: remoteModelId,
            policy: existingProfile?.policy ?? RoutingPolicy(
                escalateIfAttachment: true,
                preferLocalWhenOffline: true,
                escalateIfMessageLongerThan: 4000
            ),
            preprocessing: existingProfile?.preprocessing ?? LocalPreprocessing(
                compressHistory: false,
                rewriteRagQuery: false
            )
        )
    }

    private var localCapabilitySummary: String {
        let cap = appState.localDeviceCapability
        let installed = appState.localModels.filter { $0.downloaded && $0.verified }.count
        let installedLabel = installed == 1 ? "1 installed" : "\(installed) installed"
        if cap.maxModelBytes == 0 {
            return cap.reason ?? "Local inference is unavailable on this device"
        }
        return "\(cap.abi) • \(localBytes(cap.totalRamBytes)) RAM • \(installedLabel)"
    }

    private func localProgressLabel(_ stage: String) -> String {
        switch stage {
        case "verifying":
            return "Verifying"
        case "complete":
            return "Complete"
        case "failed":
            return "Failed"
        default:
            return "Downloading"
        }
    }

    private func localProgressBytes(_ downloaded: UInt64, _ total: UInt64?) -> String {
        if let total {
            return "\(localBytes(downloaded)) / \(localBytes(total))"
        }
        return localBytes(downloaded)
    }

    private func localBytes(_ bytes: UInt64) -> String {
        let value = Double(bytes)
        if value >= 1_073_741_824 {
            return String(format: "%.1f GiB", value / 1_073_741_824)
        }
        if value >= 1_048_576 {
            return String(format: "%.1f MiB", value / 1_048_576)
        }
        if value >= 1024 {
            return String(format: "%.1f KiB", value / 1024)
        }
        return "\(bytes) B"
    }

    private func compactModelName(_ modelId: String) -> String {
        String(modelId.split(separator: "/").last ?? Substring(modelId))
            .replacingOccurrences(of: "_", with: " ")
            .replacingOccurrences(of: "-", with: " ")
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
