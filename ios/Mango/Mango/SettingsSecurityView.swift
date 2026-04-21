import SwiftUI

struct SettingsSecurityView: View {
    @EnvironmentObject var appManager: AppManager

    @State private var duressPin: String = ""
    @State private var confirmDuressPin: String = ""
    @State private var message: String? = nil
    @State private var showDeleteChatsConfirmation = false
    @State private var showDeleteDataConfirmation = false

    private let lockTimeoutOptions: [(String, Int64)] = [
        ("Immediately", 0),
        ("1 minute", 60),
        ("5 minutes", 300),
        ("15 minutes", 900),
        ("Never", -1),
    ]

    var appState: AppState { appManager.appState }

    var body: some View {
        NavigationStack {
            List {
                lockSection
                biometricSection
                duressSection
                deleteChatsSection
                deleteDataSection
            }
            .navigationTitle("Security")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Back") { appManager.dispatch(.popScreen) }
                }
            }
            .onChange(of: appState.toast) { _, toast in
                if let toast {
                    message = toast
                    appManager.dispatch(.clearToast)
                }
            }
            .alert("Delete All Chats", isPresented: $showDeleteChatsConfirmation) {
                Button("Cancel", role: .cancel) {}
                Button("Delete", role: .destructive) {
                    appManager.dispatch(.deleteAllConversations)
                }
            } message: {
                Text("This will permanently delete every conversation and message on this device.")
            }
            .alert("Delete All Data", isPresented: $showDeleteDataConfirmation) {
                Button("Cancel", role: .cancel) {}
                Button("Delete Everything", role: .destructive) {
                    appManager.dispatch(.deleteAllData)
                }
            } message: {
                Text("This will permanently delete chats, documents, memories, API keys, auth data, and local files, then return the app to clean-install state.")
            }
        }
    }

    private var lockSection: some View {
        Section("Lock") {
            Picker("Lock Timeout", selection: Binding(
                get: { appState.lockTimeoutSeconds },
                set: { appManager.dispatch(.setLockTimeout(seconds: $0)) }
            )) {
                ForEach(lockTimeoutOptions, id: \.1) { label, seconds in
                    Text(label).tag(seconds)
                }
            }

            if appState.lockTimeoutSeconds == -1 {
                Text("Auto-lock disabled. The app will open without your PIN — it is protected only by your device unlock. If your device is unlocked, anyone with access can open the app.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
    }

    private var biometricSection: some View {
        Section("Biometric Login") {
            Toggle(isOn: Binding(
                get: { appState.biometricLoginEnabled },
                set: { appManager.dispatch(.setBiometricLoginEnabled(enabled: $0)) }
            )) {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Use Face ID / Touch ID")
                    Text(
                        appState.biometricAvailable
                            ? "Unlock with device biometrics when available."
                            : "Biometrics are not available or not enrolled on this device."
                    )
                    .font(.caption)
                    .foregroundStyle(.secondary)
                }
            }
            .disabled(!appState.biometricAvailable)
        }
    }

    private var duressSection: some View {
        Section("Duress PIN") {
            VStack(alignment: .leading, spacing: 8) {
                Text("Entering this PIN on the lock screen silently erases all local data.")
                    .font(.caption)
                    .foregroundStyle(.secondary)

                if appState.duressPinConfigured {
                    Text("A duress PIN is currently configured.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }

                SecureField("New duress PIN", text: $duressPin)
                    .textFieldStyle(.roundedBorder)
                SecureField("Confirm duress PIN", text: $confirmDuressPin)
                    .textFieldStyle(.roundedBorder)

                if let message {
                    Text(message)
                        .font(.caption)
                        .foregroundStyle(message.localizedCaseInsensitiveContains("must") || message.localizedCaseInsensitiveContains("failed") ? .red : .secondary)
                }

                Button(appState.duressPinConfigured ? "Update Duress PIN" : "Save Duress PIN") {
                    let trimmed = duressPin.trimmingCharacters(in: .whitespacesAndNewlines)
                    if trimmed.isEmpty {
                        message = "Enter a duress PIN or use Remove."
                    } else if trimmed != confirmDuressPin.trimmingCharacters(in: .whitespacesAndNewlines) {
                        message = "Duress PIN confirmation does not match."
                    } else {
                        message = nil
                        appManager.dispatch(.setDuressPin(pin: trimmed))
                        duressPin = ""
                        confirmDuressPin = ""
                    }
                }
                .buttonStyle(.borderedProminent)

                if appState.duressPinConfigured {
                    Button("Remove Duress PIN", role: .destructive) {
                        message = nil
                        appManager.dispatch(.setDuressPin(pin: nil))
                        duressPin = ""
                        confirmDuressPin = ""
                    }
                }
            }
            .padding(.vertical, 4)
        }
    }

    private var deleteChatsSection: some View {
        Section("Delete All Chats") {
            VStack(alignment: .leading, spacing: 8) {
                Text("Remove every conversation and message stored on this device.")
                    .font(.caption)
                    .foregroundStyle(.secondary)

                Button("Delete All Chats", role: .destructive) {
                    showDeleteChatsConfirmation = true
                }
            }
            .padding(.vertical, 4)
        }
    }

    private var deleteDataSection: some View {
        Section("Delete All Data") {
            VStack(alignment: .leading, spacing: 8) {
                Text("Erase chats, documents, memories, API keys, auth data, and local files, then return to the first-launch app state.")
                    .font(.caption)
                    .foregroundStyle(.secondary)

                Button("Delete All Data", role: .destructive) {
                    showDeleteDataConfirmation = true
                }
            }
            .padding(.vertical, 4)
        }
    }
}
