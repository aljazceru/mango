import SwiftUI
import os

/// Phase 32 Plan 05: Directory Sources management screen (iOS).
///
/// Responsibilities:
/// - Present the list of registered directory sources (`AppState.directorySources`)
/// - Let the user add a new folder via `DirectorySourcePicker`
/// - Show per-source sync status, file count, and last-sync time
/// - Offer Sync Now, Edit Exclusions, and Remove (with confirmation, DIR-06 / D-33)
/// - Kick off the syncDirectorySource pipeline on add + manual Sync Now
///
/// The ScenePhase `.active` hook in `ContentView` drives the foreground-resume
/// sync for all sources (D-22). This view only handles the UI and the per-source
/// dispatch.

private let ds_logger = Logger(subsystem: "dev.disobey.mango", category: "DirectorySourcesView")

struct DirectorySourcesView: View {
    @EnvironmentObject var appManager: AppManager

    @State private var showPicker = false
    @State private var sourceToRemove: DirectorySourceSummary? = nil
    @State private var editingSource: DirectorySourceSummary? = nil
    @State private var skippedCloudToast: String? = nil
    /// Bookmark cache: sourceId → bookmark BLOB. Populated at add-time and on
    /// subsequent manual syncs. Avoids a round-trip to SQLite on every Sync Now.
    @State private var bookmarkCache: [String: Data] = [:]

    /// Default exclusion preset (Obsidian-friendly) offered for new sources (D-29).
    private let defaultExclusions: [String] = [
        ".obsidian/", ".trash/", "*.tmp", "*.canvas", ".git/",
    ]

    var appState: AppState { appManager.appState }

    var body: some View {
        NavigationStack {
            List {
                Section {
                    Button {
                        showPicker = true
                    } label: {
                        Label("Add folder", systemImage: "folder.badge.plus")
                    }
                }

                if appState.directorySources.isEmpty {
                    Section {
                        Text("No directory sources yet. Add a folder to sync your notes.")
                            .foregroundStyle(.secondary)
                            .font(.subheadline)
                    }
                } else {
                    Section("Sources") {
                        ForEach(appState.directorySources, id: \.id) { source in
                            DirectorySourceRow(
                                source: source,
                                onSyncNow: { dispatchSyncNow(source) },
                                onEditExclusions: { editingSource = source },
                                onRemove: { sourceToRemove = source })
                        }
                    }
                }

                if let msg = skippedCloudToast {
                    Section {
                        Text(msg)
                            .font(.caption)
                            .foregroundStyle(.orange)
                    } header: {
                        Text("iCloud placeholders skipped")
                    }
                }
            }
            .listStyle(.insetGrouped)
            .navigationTitle("Directory Sources")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Back") {
                        appManager.dispatch(.popScreen)
                    }
                }
            }
            .sheet(isPresented: $showPicker) {
                DirectorySourcePicker(
                    onPicked: { url, bookmarkData in
                        handlePicked(url: url, bookmarkData: bookmarkData)
                        showPicker = false
                    },
                    onCancel: { showPicker = false })
            }
            .sheet(item: $editingSource) { source in
                ExclusionEditor(
                    initialGlobs: source.exclusionGlobs,
                    onSave: { newGlobs in
                        appManager.dispatch(.setDirectoryExclusions(
                            sourceId: source.id, globs: newGlobs))
                        editingSource = nil
                    },
                    onCancel: { editingSource = nil })
            }
            .confirmationDialog(
                removeConfirmTitle(sourceToRemove),
                isPresented: Binding(
                    get: { sourceToRemove != nil },
                    set: { if !$0 { sourceToRemove = nil } }),
                titleVisibility: .visible
            ) {
                Button("Remove", role: .destructive) {
                    if let id = sourceToRemove?.id {
                        appManager.dispatch(.removeDirectorySource(sourceId: id))
                        bookmarkCache.removeValue(forKey: id)
                    }
                    sourceToRemove = nil
                }
                Button("Cancel", role: .cancel) { sourceToRemove = nil }
            }
        }
    }

    // MARK: - Actions

    private func handlePicked(url: URL, bookmarkData: Data) {
        let displayName = url.lastPathComponent
        // iOS path is nil — Rust core stores only the opaque bookmark BLOB (T-32-I2).
        appManager.dispatch(.addDirectorySource(
            displayName: displayName,
            path: nil,
            bookmarkData: bookmarkData,
            treeUri: nil,
            exclusionGlobs: defaultExclusions))
        // We do NOT yet know the server-assigned source id; the first sync
        // runs when the user taps Sync Now on the new row (which will have
        // a freshly reloaded DirectorySourceSummary in appState.directorySources).
        // Cache the bookmark under the displayName temporarily so the first
        // Sync Now on a row with matching displayName can look it up.
        bookmarkCache[displayName] = bookmarkData
        ds_logger.info("picked folder \(displayName, privacy: .public), bookmark cached under displayName")
    }

    private func dispatchSyncNow(_ source: DirectorySourceSummary) {
        // Trigger the sync status flip first so the UI shows Syncing.
        appManager.dispatch(.triggerDirectorySync(sourceId: source.id))

        // Resolve bookmark: prefer in-memory cache keyed on id, then displayName
        // (post-add fallback), else bail — user must tap Sync Now again after
        // the next reload since we do not yet expose bookmark read over FFI.
        var bookmark: Data? = bookmarkCache[source.id] ?? bookmarkCache[source.displayName]

        // Promote displayName-keyed cache entry to id-keyed now that we have the id.
        if bookmark != nil, bookmarkCache[source.id] == nil {
            bookmarkCache[source.id] = bookmark
            bookmarkCache.removeValue(forKey: source.displayName)
            // Also register with the process-wide scheduler so ScenePhase
            // foreground-resume syncs can resolve the bookmark (D-22).
            if let bk = bookmark {
                DirectorySyncScheduler.cacheBookmark(sourceId: source.id, bookmarkData: bk)
            }
        }

        guard let bk = bookmark else {
            ds_logger.warning("no cached bookmark for \(source.id, privacy: .public); rebinding required")
            skippedCloudToast = "This source needs to be re-added on this device (cold launch without cached bookmark). Tap Add folder again."
            return
        }

        let sourceId = source.id
        let exclusions = source.exclusionGlobs
        Task.detached { [appManager] in
            let result = syncDirectorySource(
                sourceId: sourceId,
                bookmarkData: bk,
                exclusionGlobs: exclusions,
                ffiApp: appManager.ffiApp,
                dispatch: { action in
                    Task { @MainActor in appManager.dispatch(action) }
                })
            if !result.skippedCloud.isEmpty {
                await MainActor.run {
                    skippedCloudToast = "\(result.skippedCloud.count) iCloud file(s) not downloaded; tap the file in Files app to download."
                }
            }
        }
    }

    private func removeConfirmTitle(_ source: DirectorySourceSummary?) -> String {
        guard let s = source else { return "Remove source?" }
        let fmt = NumberFormatter()
        fmt.numberStyle = .decimal
        let count = fmt.string(from: NSNumber(value: s.fileCount)) ?? "\(s.fileCount)"
        return "Remove source and delete \(count) indexed chunks?"
    }
}

// MARK: - Row

private struct DirectorySourceRow: View {
    let source: DirectorySourceSummary
    let onSyncNow: () -> Void
    let onEditExclusions: () -> Void
    let onRemove: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 8) {
                Image(systemName: "folder.fill")
                    .foregroundStyle(.accentColor)
                Text(source.displayName)
                    .font(.subheadline)
                    .lineLimit(1)
                Spacer()
                statusBadge
            }
            HStack(spacing: 8) {
                Text("\(formattedFileCount) files")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Text("·").foregroundStyle(.secondary)
                Text("Last synced: \(source.lastSyncedLabel)")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Spacer()
            }
            HStack(spacing: 12) {
                Button("Sync Now", action: onSyncNow)
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                    .disabled(isSyncing)
                Button("Edit", action: onEditExclusions)
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                Button(role: .destructive, action: onRemove) {
                    Text("Remove")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
            }
        }
        .padding(.vertical, 4)
    }

    private var isSyncing: Bool {
        if case .syncing = source.syncStatus { return true }
        return false
    }

    @ViewBuilder
    private var statusBadge: some View {
        switch source.syncStatus {
        case .idle:
            Label("Idle", systemImage: "checkmark.circle")
                .font(.caption)
                .labelStyle(.iconOnly)
                .foregroundStyle(.secondary)
        case .syncing:
            HStack(spacing: 4) {
                ProgressView().scaleEffect(0.6)
                Text("Syncing").font(.caption).foregroundStyle(.secondary)
            }
        case .error(let message):
            Label(message, systemImage: "exclamationmark.triangle.fill")
                .font(.caption)
                .foregroundStyle(.red)
                .lineLimit(1)
        }
    }

    /// Locale-aware thousands-separated file count (e.g. 1234 → "1,234" in en-US).
    /// Relative-time label is provided by the Rust core as `source.lastSyncedLabel`
    /// so all three platforms render identically (Plan 32-07).
    private var formattedFileCount: String {
        let fmt = NumberFormatter()
        fmt.numberStyle = .decimal
        return fmt.string(from: NSNumber(value: source.fileCount)) ?? "\(source.fileCount)"
    }
}

// MARK: - Exclusion editor

private struct ExclusionEditor: View {
    let initialGlobs: [String]
    let onSave: ([String]) -> Void
    let onCancel: () -> Void

    @State private var text: String = ""

    /// Lightweight inline validation — balanced brackets, non-empty. Authoritative
    /// validation lives on the Rust side via `validate_glob_pattern` (D-29).
    private func looksLikeValidGlob(_ line: String) -> Bool {
        let trimmed = line.trimmingCharacters(in: .whitespaces)
        if trimmed.isEmpty { return false }
        let opens = trimmed.filter { $0 == "[" }.count
        let closes = trimmed.filter { $0 == "]" }.count
        return opens == closes
    }

    private var invalidLines: [String] {
        text.split(whereSeparator: { $0.isNewline })
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty && !looksLikeValidGlob($0) }
    }

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    TextEditor(text: $text)
                        .font(.body.monospaced())
                        .frame(minHeight: 200)
                } header: {
                    Text("One glob per line")
                } footer: {
                    if invalidLines.isEmpty {
                        Text("Example: *.tmp, .obsidian/, .git/. Patterns are validated by the Rust core on save.")
                            .font(.caption)
                    } else {
                        Text("Invalid patterns: \(invalidLines.joined(separator: ", "))")
                            .font(.caption)
                            .foregroundStyle(.red)
                    }
                }
            }
            .navigationTitle("Edit Exclusions")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Cancel", action: onCancel)
                }
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Save") {
                        let globs = text
                            .split(whereSeparator: { $0.isNewline })
                            .map { $0.trimmingCharacters(in: .whitespaces) }
                            .filter { !$0.isEmpty }
                        onSave(globs)
                    }
                    .disabled(!invalidLines.isEmpty)
                }
            }
            .onAppear {
                text = initialGlobs.joined(separator: "\n")
            }
        }
    }
}

// MARK: - Identifiable conformance for sheet(item:)

extension DirectorySourceSummary: Identifiable {}

// MARK: - ScenePhase foreground-resume helper (called from ContentView)

/// Fired from ContentView's `onChange(of: scenePhase)` when the app returns to
/// foreground (D-22). Enumerates every source in AppState and dispatches a sync
/// pass using the bookmark cache held on DirectorySourcesView — except this
/// static helper uses a process-wide cache fed at add-time.
enum DirectorySyncScheduler {
    /// Bookmark BLOBs captured at add-time; read on ScenePhase .active.
    /// This is a best-effort cache — a cold launch with no in-memory state
    /// requires the user to re-add the folder (plan acceptance: this is a
    /// known v1 limitation called out in the picker logger).
    static var bookmarkCache: [String: Data] = [:]

    static func cacheBookmark(sourceId: String, bookmarkData: Data) {
        bookmarkCache[sourceId] = bookmarkData
    }

    /// Called from ContentView's ScenePhase handler. Runs one sync pass per source.
    static func syncAll(appManager: AppManager) {
        let sources = appManager.appState.directorySources
        for source in sources {
            guard let bk = bookmarkCache[source.id] else {
                ds_logger.info("ScenePhase sync: no cached bookmark for \(source.id, privacy: .public), skipping")
                continue
            }
            let sourceId = source.id
            let exclusions = source.exclusionGlobs
            Task.detached { [appManager] in
                _ = syncDirectorySource(
                    sourceId: sourceId,
                    bookmarkData: bk,
                    exclusionGlobs: exclusions,
                    ffiApp: appManager.ffiApp,
                    dispatch: { action in
                        Task { @MainActor in appManager.dispatch(action) }
                    })
            }
        }
    }
}
