import SwiftUI

/// Onboarding wizard view: 4-step guided setup for Mango.
/// Steps: Welcome -> BackendSetup -> AttestationDemo -> ReadyToChat.
/// Per D-04 (back/next navigation), D-05-D-07 (backend setup), D-08-D-14 (attestation),
/// D-15-D-17 (ready to chat), ONBR-01 through ONBR-05.
struct OnboardingView: View {
    let step: OnboardingStep
    @EnvironmentObject var appManager: AppManager

    @State private var selectedPresetId: String = ""
    @State private var apiKeyText: String = ""
    @State private var showLearnMore: Bool = false

    private var onboarding: OnboardingState { appManager.appState.onboarding }

    var body: some View {
        VStack(spacing: 0) {
            // Progress dots
            progressDots
                .padding(.top, 32)
                .padding(.bottom, 24)

            // Step content
            ScrollView {
                VStack(spacing: 0) {
                    stepContent
                        .padding(.horizontal, 24)
                        .padding(.bottom, 32)
                }
            }
        }
        .background(Color(UIColor.systemBackground))
        .navigationBarHidden(true)
    }

    // MARK: - Progress Dots

    private var progressDots: some View {
        let stepIdx: Int = {
            switch step {
            case .welcome: return 0
            case .backendSetup: return 1
            case .attestationDemo: return 2
            case .readyToChat: return 3
            }
        }()

        return HStack(spacing: 8) {
            ForEach(0..<4, id: \.self) { i in
                Circle()
                    .fill(
                        i == stepIdx ? Color.accentColor :
                        i < stepIdx ? Color.accentColor.opacity(0.5) :
                        Color.secondary.opacity(0.3)
                    )
                    .frame(width: i == stepIdx ? 12 : 10, height: i == stepIdx ? 12 : 10)
            }
        }
    }

    // MARK: - Step Content

    @ViewBuilder
    private var stepContent: some View {
        switch step {
        case .welcome:
            welcomeStep
        case .backendSetup:
            backendSetupStep
        case .attestationDemo:
            attestationDemoStep
        case .readyToChat:
            readyToChatStep
        }
    }

    // MARK: - Step 1: Welcome

    private var welcomeStep: some View {
        VStack(alignment: .center, spacing: 16) {
            Text("Mango")
                .font(.largeTitle)
                .fontWeight(.bold)
                .multilineTextAlignment(.center)

            Text("Your conversations, provably private.")
                .font(.title3)
                .foregroundStyle(Color.accentColor)
                .multilineTextAlignment(.center)

            Text(
                "Every message is processed inside a Trusted Execution Environment " +
                "-- a sealed hardware enclave that no one can access, not even the server operator."
            )
            .font(.body)
            .foregroundStyle(.secondary)
            .multilineTextAlignment(.center)
            .padding(.top, 4)

            Button("Get Started") {
                appManager.dispatch(.nextOnboardingStep)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
            .padding(.top, 24)

            Button("Skip setup") {
                appManager.dispatch(.skipOnboarding)
            }
            .font(.subheadline)
            .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity)
    }

    // MARK: - Step 2: Backend Setup

    private var backendSetupStep: some View {
        let presets = knownProviderPresets()

        return VStack(alignment: .leading, spacing: 16) {
            Text("Choose your provider")
                .font(.title2)
                .fontWeight(.semibold)

            Text("Select a confidential inference provider and enter your API key.")
                .font(.subheadline)
                .foregroundStyle(.secondary)

            // Provider preset list
            VStack(spacing: 8) {
                ForEach(presets, id: \.id) { preset in
                    presetRow(preset)
                }
            }

            // API key field
            VStack(alignment: .leading, spacing: 4) {
                Text("API Key")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                SecureField("Enter your API key...", text: $apiKeyText)
                    .textFieldStyle(.roundedBorder)
                    .autocorrectionDisabled()
                    .textInputAutocapitalization(.never)
            }
            .padding(.top, 4)

            // Error text
            if let error = onboarding.apiKeyError {
                Text(error)
                    .font(.caption)
                    .foregroundStyle(.red)
            }

            // Validate button or spinner
            if onboarding.validatingApiKey {
                HStack(spacing: 8) {
                    ProgressView()
                        .controlSize(.small)
                    Text("Validating...")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }
                .padding(.top, 4)
            } else {
                Button("Validate & Continue") {
                    let trimmedKey = apiKeyText.trimmingCharacters(in: .whitespaces)
                    guard !selectedPresetId.isEmpty, !trimmedKey.isEmpty else { return }
                    // Add the backend from preset then validate
                    appManager.dispatch(.addBackendFromPreset(presetId: selectedPresetId, apiKey: trimmedKey))
                    appManager.dispatch(.validateApiKey(backendId: selectedPresetId))
                }
                .buttonStyle(.borderedProminent)
                .disabled(selectedPresetId.isEmpty || apiKeyText.trimmingCharacters(in: .whitespaces).isEmpty)
                .padding(.top, 4)
            }

            Spacer(minLength: 16)

            // Navigation row: Back and Skip
            HStack {
                Button("Back") {
                    appManager.dispatch(.previousOnboardingStep)
                }
                .font(.subheadline)
                .foregroundStyle(.secondary)

                Spacer()

                Button("Skip for now") {
                    appManager.dispatch(.skipOnboarding)
                }
                .font(.subheadline)
                .foregroundStyle(.secondary)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    @ViewBuilder
    private func presetRow(_ preset: ProviderPreset) -> some View {
        let isSelected = preset.id == selectedPresetId

        Button {
            selectedPresetId = preset.id
        } label: {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text(preset.name)
                        .font(.body)
                        .fontWeight(isSelected ? .semibold : .regular)
                        .foregroundStyle(isSelected ? Color.accentColor : .primary)
                    Text(preset.description)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                Text(teeTypeLabel(preset.teeType))
                    .font(.caption)
                    .foregroundStyle(isSelected ? Color.accentColor : .secondary)
                if isSelected {
                    Image(systemName: "checkmark.circle.fill")
                        .foregroundStyle(Color.accentColor)
                }
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 10)
            .background(isSelected ? Color.accentColor.opacity(0.1) : Color(UIColor.secondarySystemBackground))
            .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .stroke(isSelected ? Color.accentColor : Color.clear, lineWidth: 1.5)
            )
        }
        .buttonStyle(.plain)
    }

    // MARK: - Step 3: Attestation Demo

    private var attestationDemoStep: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Verifying your backend")
                .font(.title2)
                .fontWeight(.semibold)

            // Stage indicator
            if let stage = onboarding.attestationStage {
                HStack(spacing: 8) {
                    ProgressView()
                        .controlSize(.small)
                    Text(stage)
                        .font(.subheadline)
                        .foregroundStyle(Color.accentColor)
                }
            }

            // Result area
            if let result = onboarding.attestationResult {
                attestationResultView(result)
            } else {
                Text("Starting verification...")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)

                Spacer(minLength: 16)
                backButton
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    @ViewBuilder
    private func attestationResultView(_ result: AttestationStatus) -> some View {
        switch result {
        case .verified:
            VStack(alignment: .leading, spacing: 12) {
                // Verified badge
                HStack(spacing: 8) {
                    Image(systemName: "checkmark.shield.fill")
                        .foregroundStyle(.green)
                        .font(.title3)
                    Text("VERIFIED -- \(onboarding.attestationTeeLabel ?? "TEE Verified")")
                        .font(.subheadline)
                        .fontWeight(.medium)
                        .foregroundStyle(.green)
                }

                // Vault metaphor (D-13)
                Text(
                    "Think of a Trusted Execution Environment like a sealed, tamper-proof vault " +
                    "inside the server. Your data goes in, the AI processes it, and the result " +
                    "comes out -- but nobody (not even the server operator) can see what's inside. " +
                    "Attestation is the cryptographic proof that the vault is real and hasn't " +
                    "been tampered with."
                )
                .font(.subheadline)
                .foregroundStyle(.secondary)

                // Learn More (D-14)
                DisclosureGroup("Learn More", isExpanded: $showLearnMore) {
                    VStack(alignment: .leading, spacing: 8) {
                        Group {
                            Text("What is a TEE?")
                                .fontWeight(.semibold)
                            Text(
                                "A Trusted Execution Environment is a hardware-isolated region of a processor. " +
                                "Code and data inside the TEE cannot be read or modified by the operating system, " +
                                "hypervisor, or server administrator."
                            )
                        }
                        Group {
                            Text("What does attestation prove?")
                                .fontWeight(.semibold)
                            Text(
                                "Attestation is a cryptographic certificate from the hardware itself. It proves: " +
                                "(1) the TEE is genuine hardware, not a simulation, " +
                                "(2) the software running inside hasn't been tampered with, " +
                                "(3) your data is being processed in the secure enclave right now."
                            )
                        }
                        Group {
                            Text("Self-verified vs Provider-verified:")
                                .fontWeight(.semibold)
                            Text(
                                "Self-verified means this app checked the cryptographic proof directly. " +
                                "Provider-verified means the backend's own attestation service confirmed the TEE. " +
                                "Both guarantee your data is protected."
                            )
                        }
                    }
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .padding(.top, 8)
                }
                .font(.subheadline)
                .foregroundStyle(Color.accentColor)

                Button("Next") {
                    appManager.dispatch(.nextOnboardingStep)
                }
                .buttonStyle(.borderedProminent)
                .padding(.top, 8)

                backButton
            }

        case .failed:
            VStack(alignment: .leading, spacing: 12) {
                HStack(spacing: 8) {
                    Image(systemName: "xmark.shield.fill")
                        .foregroundStyle(.red)
                        .font(.title3)
                    Text("Verification failed")
                        .font(.subheadline)
                        .fontWeight(.medium)
                        .foregroundStyle(.red)
                }

                Button("Retry") {
                    appManager.dispatch(.validateApiKey(backendId: appManager.appState.onboarding.selectedBackendId ?? ""))
                }
                .buttonStyle(.bordered)

                Button("Continue anyway") {
                    appManager.dispatch(.nextOnboardingStep)
                }
                .font(.subheadline)
                .foregroundStyle(.secondary)

                backButton
            }

        default:
            VStack(alignment: .leading, spacing: 12) {
                Text("Waiting for attestation result...")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                backButton
            }
        }
    }

    // MARK: - Step 4: Ready to Chat

    private var readyToChatStep: some View {
        VStack(alignment: .center, spacing: 16) {
            Image(systemName: "checkmark.circle.fill")
                .font(.system(size: 56))
                .foregroundStyle(.green)

            Text("You're ready!")
                .font(.largeTitle)
                .fontWeight(.bold)
                .multilineTextAlignment(.center)

            Text("Your backend is verified and ready. Start a confidential conversation.")
                .font(.body)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)

            // Start Chatting (D-16)
            Button("Start Chatting") {
                appManager.dispatch(.completeOnboarding)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
            .padding(.top, 24)

            backButton
                .padding(.top, 8)
        }
        .frame(maxWidth: .infinity)
    }

    // MARK: - Shared Components

    private var backButton: some View {
        Button("Back") {
            appManager.dispatch(.previousOnboardingStep)
        }
        .font(.subheadline)
        .foregroundStyle(.secondary)
    }

    // MARK: - Helpers

    private func teeTypeLabel(_ teeType: TeeType) -> String {
        switch teeType {
        case .intelTdx: return "Intel TDX"
        case .nvidiaH100Cc: return "NVIDIA H100 CC"
        case .amdSevSnp: return "AMD SEV-SNP"
        case .unknown: return "Unknown"
        }
    }
}
