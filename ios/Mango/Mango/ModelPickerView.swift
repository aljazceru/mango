import SwiftUI

/// Inline model picker shown in the chat header.
/// Presents available models from the active backend via a menu.
struct ModelPickerView: View {
    let backends: [BackendSummary]
    let activeBackendId: String?
    let selectedModelId: String?
    let onSelectModel: (String) -> Void

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
        } label: {
            HStack(spacing: 4) {
                Text(currentModelName)
                    .font(.subheadline)
                    .foregroundColor(.secondary)
                Image(systemName: "chevron.up.chevron.down")
                    .font(.caption2)
                    .foregroundColor(.secondary)
            }
        }
        .accessibilityLabel("Model: \(currentModelName)")
    }

    private var availableModels: [ModelInfo] {
        guard let backendId = activeBackendId,
              let backend = backends.first(where: { $0.id == backendId }) else {
            return []
        }
        return backend.availableModels.map { modelId in
            ModelInfo(id: modelId, displayName: shortModelName(modelId))
        }
    }

    private var currentModelName: String {
        guard let modelId = selectedModelId else { return "Model" }
        return shortModelName(modelId)
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
