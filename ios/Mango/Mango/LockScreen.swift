import SwiftUI

/// iOS lock gate screen (Phase 28, D-09, D-11, D-24).
///
/// Shown on every cold launch and after background timeout (D-10).
/// Presents biometric prompt automatically on appear if biometric login is enabled (D-11).
/// Falls back to PIN/password entry (mandatory per D-24 — biometrics are additive, not required).
///
/// Security (T-28-18): PIN input uses SecureField (masked). The entered string is dispatched
/// immediately to the actor and not retained in view state after dispatch.
///
/// No "forgot PIN" link per the security model (D-15): recovery = reinstall.
struct LockScreen: View {
    @EnvironmentObject var appManager: AppManager

    @State private var enteredPin: String = ""
    @State private var pinFieldFocused: Bool = false

    private var appState: AppState { appManager.appState }

    var body: some View {
        VStack(spacing: 0) {
            Spacer()

            // App icon / wordmark at top
            appBranding

            Spacer().frame(height: 48)

            // PIN/password input
            pinInputSection

            Spacer()

            // Footer note
            Text("No \"Forgot PIN\" option — recovery requires reinstalling the app.")
                .font(.caption2)
                .foregroundStyle(.tertiary)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)
                .padding(.bottom, 24)
        }
        .background(Color(.systemBackground))
        .onAppear {
            // D-11: auto-prompt biometrics on appear if biometric login is enabled.
            if appState.biometricLoginEnabled {
                attemptBiometricUnlock()
            }
        }
    }

    // MARK: - Subviews

    private var appBranding: some View {
        VStack(spacing: 16) {
            Image(systemName: "lock.shield.fill")
                .font(.system(size: 64))
                .foregroundStyle(.tint)

            Text("Mango")
                .font(.largeTitle.weight(.bold))

            Text("Enter your PIN to unlock")
                .font(.subheadline)
                .foregroundStyle(.secondary)
        }
    }

    private var pinInputSection: some View {
        VStack(spacing: 16) {
            // T-28-18: SecureField masks PIN input at the view layer.
            SecureField("Enter PIN or password", text: $enteredPin)
                .textFieldStyle(.roundedBorder)
                .keyboardType(.default)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled(true)
                .submitLabel(.done)
                .onSubmit { submitPin() }
                .padding(.horizontal, 32)

            // Unlock button
            Button(action: submitPin) {
                Text("Unlock")
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 14)
            }
            .buttonStyle(.borderedProminent)
            .disabled(enteredPin.isEmpty)
            .padding(.horizontal, 32)

            // Biometric button (only shown if enabled)
            if appState.biometricLoginEnabled {
                Button(action: attemptBiometricUnlock) {
                    Label("Use Face ID / Touch ID", systemImage: "faceid")
                        .font(.callout)
                }
                .buttonStyle(.borderless)
                .padding(.top, 4)
            }
        }
    }

    // MARK: - Actions

    private func submitPin() {
        guard !enteredPin.isEmpty else { return }
        let pin = enteredPin
        enteredPin = ""
        appManager.dispatch(.unlockWithPin(pin: pin))
    }

    private func attemptBiometricUnlock() {
        appManager.dispatch(.attemptBiometricUnlock)
    }
}
