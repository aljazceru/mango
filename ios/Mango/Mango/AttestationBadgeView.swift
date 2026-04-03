import SwiftUI

/// Capsule-shaped attestation trust badge shown in the chat header.
/// Color and icon reflect the attestation status per the UI-SPEC color contract.
struct AttestationBadgeView: View {
    let status: AttestationStatus
    @State private var showDetailSheet = false
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        Button(action: { showDetailSheet = true }) {
            HStack(spacing: 4) {
                Image(systemName: iconName)
                    .font(.caption)
                Text(label)
                    .font(.caption)
                    .fontWeight(.medium)
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(badgeBackground)
            .foregroundColor(badgeForeground)
            .clipShape(Capsule())
            .overlay(
                Capsule()
                    .stroke(badgeStroke, lineWidth: 1)
            )
        }
        .buttonStyle(.plain)
        .accessibilityLabel("Attestation status: \(label)")
        .sheet(isPresented: $showDetailSheet) {
            AttestationDetailSheet(status: status, onDismiss: { showDetailSheet = false })
                .presentationDetents([.medium])
        }
    }

    private var label: String {
        switch status {
        case .verified:
            return "Verified"
        case .unverified:
            return "Not Verified"
        case .expired:
            return "Expired"
        case .failed:
            return "Failed"
        }
    }

    private var iconName: String {
        switch status {
        case .verified:
            return "checkmark.shield.fill"
        case .unverified:
            return "questionmark.circle"
        case .expired:
            return "clock.fill"
        case .failed:
            return "exclamationmark.triangle.fill"
        }
    }

    private var badgeBackground: Color {
        AppColors.attestBg(status, colorScheme)
    }

    private var badgeForeground: Color {
        AppColors.attestText(status, colorScheme)
    }

    private var badgeStroke: Color {
        AppColors.attestBorder(status, colorScheme)
    }
}

// MARK: - Detail Sheet

private struct AttestationDetailSheet: View {
    let status: AttestationStatus
    let onDismiss: () -> Void

    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 16) {
                Text(detailText)
                    .font(.body)
                    .foregroundColor(.primary)
                Spacer()
            }
            .padding(16)
            .navigationTitle("Trust Status")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done", action: onDismiss)
                }
            }
        }
    }

    private var detailText: String {
        switch status {
        case .verified:
            return "This conversation is routed to a Trusted Execution Environment. The TEE attestation report has been independently verified by this app."
        case .unverified:
            return "Attestation has not been checked for this backend yet."
        case .expired:
            return "The cached attestation result has expired and re-verification is needed."
        case .failed(let reason):
            return "Attestation verification failed. The backend could not prove it is running in a trusted environment.\n\nReason: \(reason)"
        }
    }
}
