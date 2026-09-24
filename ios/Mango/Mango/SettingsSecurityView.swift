import SwiftUI

struct SettingsSecurityView: View {
    @EnvironmentObject var appManager: AppManager

    @State private var duressPin: String = ""
    @State private var confirmDuressPin: String = ""
    @State private var message: String? = nil
    @State private var showDeleteChatsConfirmation = false
    @State private var showDeleteDataConfirmation = false
    @State private var retentionDaysText: String = ""

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
                retentionSection
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

    private var retentionSection: some View {
        Section("Conversation Retention") {
            Picker("Mode", selection: Binding(
                get: { appState.conversationRetentionMode },
                set: { mode in
                    appManager.dispatch(.setConversationRetention(
                        mode: mode,
                        days: appState.conversationRetentionDays
                    ))
                }
            )) {
                Text("Off").tag(ConversationRetentionMode.off)
                Text("Archive").tag(ConversationRetentionMode.archive)
                Text("Delete").tag(ConversationRetentionMode.delete)
            }

            HStack {
                TextField(
                    "days",
                    text: Binding(
                        get: { retentionDaysText.isEmpty ? String(appState.conversationRetentionDays) : retentionDaysText },
                        set: { retentionDaysText = $0.filter { $0.isNumber } }
                    )
                )
                .keyboardType(.numberPad)
                .textFieldStyle(.roundedBorder)
                Button("Apply") {
                    if let days = UInt32(retentionDaysText), days > 0 {
                        appManager.dispatch(.setConversationRetention(
                            mode: appState.conversationRetentionMode,
                            days: days
                        ))
                    }
                    retentionDaysText = ""
                }
                .buttonStyle(.bordered)
            }

            Text("Automatically archive or delete conversations older than the given number of days. Archived conversations are hidden from the chat list and can be restored below. Off by default.")
                .font(.caption)
                .foregroundStyle(.secondary)

            if appState.conversationRetentionMode == .delete {
                Text("Delete permanently removes conversations and their messages once they pass the threshold. This cannot be undone.")
                    .font(.caption)
                    .foregroundStyle(.red)
            }

            if !appState.archivedConversations.isEmpty {
                ForEach(appState.archivedConversations, id: \.id) { conv in
                    HStack {
                        Text(conv.title)
                            .lineLimit(1)
                        Spacer()
                        Button("Unarchive") {
                            appManager.dispatch(.unarchiveConversation(id: conv.id))
                        }
                        .buttonStyle(.bordered)
                    }
                }
            }
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
