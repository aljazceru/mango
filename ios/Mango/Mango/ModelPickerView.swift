import SwiftUI

/// Inline model picker shown in the chat header.
/// Presents available models from the active backend via a menu.
/// Shows a small colored dot next to the model name to indicate attestation status
/// (green = verified, yellow = expired, red = failed, none = unverified).
struct ModelPickerView: View {
    let backends: [BackendSummary]
    let activeBackendId: String?
    let selectedModelId: String?
    var hybridProfiles: [HybridProfile] = []
    var activeHybridProfileId: String? = nil
    var attestationStatus: AttestationStatus? = nil
    let onSelectModel: (String) -> Void
    var onUseHybridProfile: (String) -> Void = { _ in }

    var body: some View {
        Menu {
            ForEach(availableModels, id: \.id) { model in
                Button(action: { onSelectModel(model.id) }) {
                    if model.id == selectedModelId {
                        Label(model.displayName, systemImage: "checkmark")
                    } else {
                        Text(model.displayName)
                    }
                }
            }
            if !hybridProfiles.isEmpty {
                Divider()
                ForEach(hybridProfiles, id: \.id) { profile in
                    Button(action: { onUseHybridProfile(profile.id) }) {
                        if profile.id == activeHybridProfileId {
                            Label(profile.name, systemImage: "checkmark")
                        } else {
                            Text(profile.name)
                        }
                    }
                }
            }
        } label: {
            HStack(spacing: 4) {
                // Attestation dot indicator: replaces the separate badge in the header
                if let dot = attestationDotColor {
                    Circle()
                        .fill(dot)
                        .frame(width: 7, height: 7)
                        .accessibilityLabel(attestationAccessibilityLabel)
                }
                Text(currentModelName)
                    .font(.subheadline)
                    .foregroundColor(.secondary)
                Image(systemName: "chevron.up.chevron.down")
                    .font(.caption2)
                    .foregroundColor(.secondary)
            }
        }
        .accessibilityLabel("Model: \(currentModelName). \(attestationAccessibilityLabel)")
    }

    /// Returns the dot color for the current attestation status.
    /// Returns nil for .unverified (no dot shown when unverified).
    private var attestationDotColor: Color? {
        guard let status = attestationStatus else { return nil }
        switch status {
        case .verified(_, _, _):  return Color(red: 0.2, green: 0.75, blue: 0.3)   // green
        case .expired:   return Color(red: 0.98, green: 0.75, blue: 0.14) // amber
        case .failed:    return Color(red: 0.9, green: 0.24, blue: 0.24)  // red
        case .unverified: return nil
        }
    }

    private var attestationAccessibilityLabel: String {
        guard let status = attestationStatus else { return "" }
        switch status {
        case .verified(_, _, _):   return "Verified"
        case .unverified: return ""
        case .expired:    return "Attestation expired"
        case .failed:     return "Attestation failed"
        }
    }

    private var availableModels: [ModelInfo] {
        guard let backendId = activeBackendId,
              let backend = backends.first(where: { $0.id == backendId }) else {
            return []
        }
        return backend.models.map { modelId in
            ModelInfo(id: modelId, displayName: shortModelName(modelId))
        }
    }

    private var currentModelName: String {
        if let profile = activeHybridProfile {
            return "Hybrid: \(profile.name)"
        }
        guard let modelId = selectedModelId else { return "Model" }
        return shortModelName(modelId)
    }

    private var activeHybridProfile: HybridProfile? {
        guard let activeHybridProfileId else { return nil }
        return hybridProfiles.first(where: { $0.id == activeHybridProfileId })
    }
}

// MARK: - Supporting Types

private struct ModelInfo: Identifiable {
    let id: String
    let displayName: String
}

// MARK: - Helpers

private func shortModelName(_ modelId: String) -> String {
    if let slash = modelId.lastIndex(of: "/") {
        return String(modelId[modelId.index(after: slash)...])
    }
    return modelId
}
