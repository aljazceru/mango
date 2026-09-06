import SwiftUI

/// iOS first-time PIN/password setup screen (Phase 28, D-14, D-18).
///
/// Shown after the onboarding wizard completes for new installs, or when the user
/// has completed onboarding but has not yet set up a PIN (auth_initialized = false).
///
/// Flow:
///   Step 1 — Set a PIN or password (minimum 4 characters, with confirmation field).
///   Step 2 — Optional duress PIN ("emergency PIN" that erases all data on entry, D-18).
///             Duress PIN must differ from the real PIN by at least 1 character.
///   Step 3 — Enable Face ID / Touch ID if biometric hardware is available (D-14).
///
/// There is no "Skip" option — encryption is always on (D-14). The Continue button
/// dispatches AppAction.setupPin which derives KEK, wraps DEK, and migrates the DB.
///
/// Security (T-28-18): All PIN fields use SecureField (masked input).
struct PinSetupScreen: View {
    @EnvironmentObject var appManager: AppManager

    // Step 1: real PIN
    @State private var pin: String = ""
    @State private var pinConfirm: String = ""
    @State private var pinError: String? = nil

    // Step 2: duress PIN (optional)
    @State private var duressPin: String = ""
    @State private var skipDuress: Bool = false
    @State private var duressError: String? = nil

    // Step 3: biometric toggle
    @State private var enableBiometric: Bool = false

    // Wizard step
    @State private var step: SetupStep = .pin

    private var appState: AppState { appManager.appState }

    /// Local validation or the last core failure (e.g. storage error,
    /// "Incorrect PIN.", or a failed enrollment). iOS has no global toast
    /// rendering, so each step surfaces this inline.
    private var currentError: String? {
        pinError ?? duressError ?? appState.toast
    }

    enum SetupStep {
        case pin, duress, biometric
    }

    /// Clear stale core and local errors before a new submission so the UI
    /// never mixes an old failure with the current attempt.
    private func clearSubmissionErrors() {
        pinError = nil
        duressError = nil
        appManager.dispatch(.clearToast)
    }

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                if !appState.enrollmentResumePending {
                    progressIndicator
                        .padding(.top, 8)
                }

                Spacer()

                if appState.enrollmentResumePending {
                    resumeStep
                } else {
                    switch step {
                    case .pin:
                        pinStep
                    case .duress:
                        duressStep
                    case .biometric:
                        biometricStep
                    }
                }

                Spacer()
            }
            .navigationTitle(stepTitle)
            .navigationBarTitleDisplayMode(.inline)
        }
    }

    // MARK: - Interrupted enrollment resume

    /// Shown when a staged pending_auth row survived a crash before the
    /// enrollment finished. Only the PIN chosen earlier can complete it — the
    /// duress/biometric choices were already captured — so this is a single
    /// field plus an explicit explanation, not a new-credential form.
    private var resumeStep: some View {
        VStack(spacing: 24) {
            VStack(alignment: .leading, spacing: 8) {
                Text("Finish Encryption Setup")
                    .font(.title2.weight(.semibold))
                Text("Setup was interrupted before encryption finished. Enter the PIN or password you chose earlier to finish protecting your data.")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 32)

            VStack(spacing: 12) {
                SecureField("PIN or password", text: $pin)
                    .textFieldStyle(.roundedBorder)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled(true)
                    .padding(.horizontal, 32)

                if let error = currentError {
                    Text(error)
                        .font(.caption)
                        .foregroundStyle(.red)
                        .padding(.horizontal, 32)
                }
            }

            Button("Resume Setup") {
                clearSubmissionErrors()
                guard !pin.isEmpty else {
                    pinError = "Enter the PIN you chose earlier."
                    return
                }
                appManager.dispatch(.setupPin(
                    pin: pin,
                    duressPin: nil,
                    enableBiometric: false
                ))
            }
            .buttonStyle(.borderedProminent)
            .frame(maxWidth: .infinity)
            .padding(.horizontal, 32)
            .disabled(pin.isEmpty)
        }
    }

    // MARK: - Step: PIN setup

    private var pinStep: some View {
        VStack(spacing: 24) {
            VStack(alignment: .leading, spacing: 8) {
                Text("Choose a PIN or password")
                    .font(.title2.weight(.semibold))
                Text("This protects your encrypted data. Minimum 4 characters.")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 32)

            VStack(spacing: 12) {
                // T-28-18: SecureField masks input at the view layer.
                SecureField("PIN or password", text: $pin)
                    .textFieldStyle(.roundedBorder)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled(true)
                    .padding(.horizontal, 32)

                SecureField("Confirm PIN or password", text: $pinConfirm)
                    .textFieldStyle(.roundedBorder)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled(true)
                    .padding(.horizontal, 32)

                if let error = currentError {
                    Text(error)
                        .font(.caption)
                        .foregroundStyle(.red)
                        .padding(.horizontal, 32)
                }
            }

            Button("Continue") {
                validateAndAdvanceFromPin()
            }
            .buttonStyle(.borderedProminent)
            .frame(maxWidth: .infinity)
            .padding(.horizontal, 32)
            .disabled(pin.isEmpty || pinConfirm.isEmpty)
        }
    }

    // MARK: - Step: Duress PIN

    private var duressStep: some View {
        VStack(spacing: 24) {
            VStack(alignment: .leading, spacing: 8) {
                Text("Set an emergency PIN?")
                    .font(.title2.weight(.semibold))
                Text("This PIN will silently erase all app data when entered. It cannot be recovered. Use it under coercion to protect your data.")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 32)

            if !skipDuress {
                VStack(spacing: 12) {
                    SecureField("Emergency PIN (must differ from your real PIN)", text: $duressPin)
                        .textFieldStyle(.roundedBorder)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled(true)
                        .padding(.horizontal, 32)
                }
            }

            if let error = currentError {
                Text(error)
                    .font(.caption)
                    .foregroundStyle(.red)
                    .padding(.horizontal, 32)
            }

            VStack(spacing: 12) {
                Button(skipDuress ? "Set an emergency PIN" : "Continue with emergency PIN") {
                    if skipDuress {
                        skipDuress = false
                    } else {
                        validateAndAdvanceFromDuress()
                    }
                }
                .buttonStyle(.borderedProminent)
                .frame(maxWidth: .infinity)
                .padding(.horizontal, 32)
                .disabled(!skipDuress && duressPin.isEmpty)

                Button(skipDuress ? "Continue without emergency PIN" : "Skip — no emergency PIN") {
                    if !skipDuress {
                        skipDuress = true
                        duressPin = ""
                        duressError = nil
                    }
                    advanceFromDuress()
                }
                .buttonStyle(.borderless)
                .foregroundStyle(.secondary)
            }
        }
    }

    // MARK: - Step: Biometric

    private var biometricStep: some View {
        VStack(spacing: 24) {
            VStack(alignment: .leading, spacing: 8) {
                Text("Enable Face ID / Touch ID?")
                    .font(.title2.weight(.semibold))
                Text("Use biometrics to unlock quickly. Your PIN remains the fallback if biometrics fail.")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 32)

            Toggle("Enable biometric unlock", isOn: $enableBiometric)
                .padding(.horizontal, 32)

            if let error = currentError {
                Text(error)
                    .font(.caption)
                    .foregroundStyle(.red)
                    .padding(.horizontal, 32)
            }

            Button("Set Up Encryption") {
                submitSetup()
            }
            .buttonStyle(.borderedProminent)
            .frame(maxWidth: .infinity)
            .padding(.horizontal, 32)
        }
    }

    // MARK: - Progress indicator

    private var progressIndicator: some View {
        HStack(spacing: 8) {
            ForEach(0..<totalSteps, id: \.self) { idx in
                Capsule()
                    .fill(idx <= currentStepIndex ? Color.accentColor : Color(.systemGray4))
                    .frame(height: 4)
            }
        }
        .padding(.horizontal, 32)
    }

    private var totalSteps: Int {
        appState.biometricAvailable ? 3 : 2
    }

    private var currentStepIndex: Int {
        switch step {
        case .pin: return 0
        case .duress: return 1
        case .biometric: return 2
        }
    }

    private var stepTitle: String {
        if appState.enrollmentResumePending { return "Resume Setup" }
        switch step {
        case .pin: return "Set Up PIN"
        case .duress: return "Emergency PIN"
        case .biometric: return "Biometrics"
        }
    }

    // MARK: - Validation & navigation

    private func validateAndAdvanceFromPin() {
        clearSubmissionErrors()
        guard pin.count >= 4 else {
            pinError = "PIN must be at least 4 characters."
            return
        }
        guard pin == pinConfirm else {
            pinError = "PINs do not match. Please try again."
            return
        }
        step = .duress
    }

    private func validateAndAdvanceFromDuress() {
        clearSubmissionErrors()
        // D-18: duress PIN must differ from the real PIN by at least 1 character.
        guard duressPin != pin else {
            duressError = "Emergency PIN must differ from your real PIN."
            return
        }
        guard duressPin.count >= 4 else {
            duressError = "Emergency PIN must be at least 4 characters."
            return
        }
        advanceFromDuress()
    }

    private func advanceFromDuress() {
        if appState.biometricAvailable {
            step = .biometric
        } else {
            submitSetup()
        }
    }

    private func submitSetup() {
        clearSubmissionErrors()
        let finalDuressPin = skipDuress || duressPin.isEmpty ? nil : duressPin
        appManager.dispatch(.setupPin(
            pin: pin,
            duressPin: finalDuressPin,
            enableBiometric: enableBiometric
        ))
    }
}
