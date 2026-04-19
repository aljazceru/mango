import SwiftUI
import UniformTypeIdentifiers
import os

/// Phase 32 Plan 05: iOS directory source picker + bookmark lifecycle + enumerator.
///
/// Responsibilities:
/// 1. Present `UIDocumentPickerViewController(forOpeningContentTypes: [.folder])` and produce
///    a security-scoped bookmark via `URL.bookmarkData(options: .minimalBookmark)`.
///    IMPORTANT: iOS does NOT accept `.withSecurityScope` — that is macOS-only (Pitfall 1).
///    The opaque Data blob returned here is stored verbatim in `directory_sources.bookmark_data`
///    via `AppAction.addDirectorySource` and never crosses the UniFFI boundary as a path
///    (T-32-I2).
/// 2. Resolve a bookmark on every subsequent sync; if `isStale`, re-create it and surface
///    a refreshed BLOB for the caller to dispatch `AppAction.updateDirectorySourceBookmark`
///    (D-14).
/// 3. Enumerate files inside the resolved directory with `FileManager.enumerator`, wrapped
///    in `startAccessingSecurityScopedResource` / `stopAccessingSecurityScopedResource`
///    pairs (D-16). Apply a simple native-side glob matcher against the stored exclusion list.
/// 4. Skip iCloud placeholder files (`ubiquitousItemDownloadingStatus == .notDownloaded`, D-17)
///    so offloaded vault files do not block the sync pipeline; the caller surfaces the skipped
///    list in the UI.

private let picker_logger = Logger(subsystem: "dev.disobey.mango", category: "DirectorySourcePicker")

// MARK: - SwiftUI wrapper around UIDocumentPickerViewController

struct DirectorySourcePicker: UIViewControllerRepresentable {
    /// Called with the resolved folder URL and the `.minimalBookmark` BLOB the caller must
    /// persist via `AppAction.addDirectorySource`.
    let onPicked: (URL, Data) -> Void
    let onCancel: () -> Void

    func makeUIViewController(context: Context) -> UIDocumentPickerViewController {
        // `.folder` + `asCopy: false` returns a security-scoped URL, not a copy in the app
        // container. This is the only iOS path to folder access (D-13).
        let picker = UIDocumentPickerViewController(forOpeningContentTypes: [.folder], asCopy: false)
        picker.delegate = context.coordinator
        picker.allowsMultipleSelection = false
        return picker
    }

    func updateUIViewController(_ uiViewController: UIDocumentPickerViewController, context: Context) {}
    func makeCoordinator() -> Coordinator { Coordinator(self) }

    final class Coordinator: NSObject, UIDocumentPickerDelegate {
        let parent: DirectorySourcePicker
        init(_ parent: DirectorySourcePicker) { self.parent = parent }

        func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
            guard let url = urls.first else { parent.onCancel(); return }
            // Security scope must be active before we call bookmarkData (D-16 / Pitfall 2).
            guard url.startAccessingSecurityScopedResource() else {
                picker_logger.warning("startAccessingSecurityScopedResource failed for picked URL")
                parent.onCancel()
                return
            }
            defer { url.stopAccessingSecurityScopedResource() }
            do {
                // `.minimalBookmark` is the iOS-correct option — `.withSecurityScope` is
                // macOS-only and crashes or fails silently on iOS (Pitfall 1).
                let data = try url.bookmarkData(options: .minimalBookmark,
                                                includingResourceValuesForKeys: nil,
                                                relativeTo: nil)
                parent.onPicked(url, data)
            } catch {
                picker_logger.error("bookmarkData failed: \(error.localizedDescription)")
                parent.onCancel()
            }
        }

        func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {
            parent.onCancel()
        }
    }
}

// MARK: - Bookmark lifecycle helpers (D-14, Pitfall 1)

enum BookmarkResolveResult {
    /// Bookmark resolved. `refreshedBookmark` is non-nil iff `isStale == true` and a
    /// fresh bookmark was successfully re-created — caller must persist it via
    /// `AppAction.updateDirectorySourceBookmark`.
    case ok(url: URL, isStale: Bool, refreshedBookmark: Data?)
    case failure(reason: String)
}

/// Resolve a persisted bookmark BLOB back into a URL with security scope available.
///
/// Callers must themselves wrap subsequent reads in `startAccessingSecurityScopedResource`
/// / `stopAccessingSecurityScopedResource`. If `bookmarkDataIsStale` is true we attempt to
/// re-create the bookmark and return the new BLOB; on success the caller MUST dispatch
/// `AppAction.updateDirectorySourceBookmark` so the SQLite row stays in sync with reality
/// (D-14).
func resolveBookmark(_ data: Data) -> BookmarkResolveResult {
    var isStale = false
    do {
        let url = try URL(resolvingBookmarkData: data,
                          options: [],
                          relativeTo: nil,
                          bookmarkDataIsStale: &isStale)
        var refreshed: Data? = nil
        if isStale {
            // Start scope before calling bookmarkData again (Pitfall 2).
            if url.startAccessingSecurityScopedResource() {
                defer { url.stopAccessingSecurityScopedResource() }
                refreshed = try? url.bookmarkData(options: .minimalBookmark,
                                                  includingResourceValuesForKeys: nil,
                                                  relativeTo: nil)
            } else {
                picker_logger.warning("resolveBookmark: start scope failed during re-create")
            }
        }
        return .ok(url: url, isStale: isStale, refreshedBookmark: refreshed)
    } catch {
        return .failure(reason: "resolve bookmark failed: \(error.localizedDescription)")
    }
}

// MARK: - Simple glob matcher

/// Deliberately simple on-device matcher — covers the Obsidian default set
/// (`.obsidian/`, `.trash/`, `*.tmp`, `*.canvas`, `.git/`). For exhaustive
/// globset-parity the Rust side re-validates every pattern on save (D-29).
struct GlobMatcher {
    let patterns: [String]

    func matches(_ relPath: String) -> Bool {
        for raw in patterns {
            let pattern = raw.trimmingCharacters(in: .whitespaces)
            if pattern.isEmpty { continue }
            if pattern.hasSuffix("/") {
                // Directory prefix match: ".obsidian/" matches ".obsidian/foo.md".
                let prefix = pattern
                if relPath.hasPrefix(prefix) { return true }
                // Also match a mid-path directory component.
                if relPath.contains("/" + prefix) { return true }
            } else if pattern.hasPrefix("*.") {
                // Extension match.
                let ext = String(pattern.dropFirst())  // ".tmp"
                if relPath.hasSuffix(ext) { return true }
            } else if pattern.hasPrefix(".") && !pattern.contains("/") {
                // Dotfile literal match (".DS_Store").
                if relPath == pattern { return true }
                if relPath.hasSuffix("/" + pattern) { return true }
            } else {
                // Literal prefix / full match.
                if relPath == pattern { return true }
                if relPath.hasPrefix(pattern) { return true }
            }
        }
        return false
    }
}

// MARK: - Enumerator (D-16, D-17, Pitfalls 2 & 3)

struct EnumerationResult {
    /// (relative_path, mtime_secs, size_bytes) triples for files that passed filtering.
    let entries: [(String, Int64, Int64)]
    /// iCloud placeholder files (not downloaded) — surfaced in the UI so the user can
    /// choose to download them (D-17).
    let skippedCloud: [String]
    /// Non-fatal errors encountered during enumeration.
    let errors: [String]
}

/// Enumerate a directory while respecting the security scope + the exclusion globs.
///
/// iCloud placeholder files are skipped: we never call `startDownloadingUbiquitousItem`
/// (Pitfall 3) because that leaks presence to iCloud servers and would block the sync on
/// large offloaded vaults.
func enumerateDirectory(rootURL: URL, exclusionGlobs: [String]) -> EnumerationResult {
    // D-16 / Pitfall 2: every read path must be wrapped.
    guard rootURL.startAccessingSecurityScopedResource() else {
        return EnumerationResult(entries: [], skippedCloud: [],
                                 errors: ["startAccessingSecurityScopedResource failed"])
    }
    defer { rootURL.stopAccessingSecurityScopedResource() }

    let fm = FileManager.default
    let keys: [URLResourceKey] = [
        .contentModificationDateKey, .fileSizeKey, .isDirectoryKey,
        .isUbiquitousItemKey, .ubiquitousItemDownloadingStatusKey,
    ]
    guard let enumerator = fm.enumerator(at: rootURL,
                                          includingPropertiesForKeys: keys,
                                          options: []) else {
        return EnumerationResult(entries: [], skippedCloud: [],
                                 errors: ["enumerator unavailable for \(rootURL.path)"])
    }

    var entries: [(String, Int64, Int64)] = []
    var skipped: [String] = []
    var errors: [String] = []
    let excludeMatcher = GlobMatcher(patterns: exclusionGlobs)
    let rootPrefix = rootURL.path + "/"

    for case let url as URL in enumerator {
        var relPath = url.path
        if relPath.hasPrefix(rootPrefix) {
            relPath = String(relPath.dropFirst(rootPrefix.count))
        }
        if excludeMatcher.matches(relPath) { continue }
        let values = try? url.resourceValues(forKeys: Set(keys))
        if values?.isDirectory == true { continue }
        // D-17 / Pitfall 3: skip iCloud placeholders without triggering a download.
        if values?.isUbiquitousItem == true,
           values?.ubiquitousItemDownloadingStatus == .notDownloaded {
            skipped.append(relPath)
            continue
        }
        let mtime = Int64(values?.contentModificationDate?.timeIntervalSince1970 ?? 0)
        let size = Int64(values?.fileSize ?? 0)
        entries.append((relPath, mtime, size))
    }
    return EnumerationResult(entries: entries, skippedCloud: skipped, errors: errors)
}

// MARK: - 50-file batching dispatch (D-25, T-32-DoS1)

/// Native-side diff against fingerprints previously stored in `directory_files`.
/// Produces the set of (added ∪ modified) entries + the list of removed relative paths.
///
/// Matches the Rust `diff_files` semantics in Plan 04: modified iff (stored.mtime != current.mtime)
/// OR (stored.size != current.size).
func diffAgainstStored(current: [(String, Int64, Int64)],
                       stored: [DirectoryFingerprint]) -> (changed: [(String, Int64, Int64)], removedPaths: [String]) {
    var storedByPath: [String: (Int64, Int64)] = [:]
    for f in stored {
        storedByPath[f.relativePath] = (f.mtimeSecs, f.sizeBytes)
    }
    var changed: [(String, Int64, Int64)] = []
    var currentPaths: Set<String> = []
    for (rel, mtime, size) in current {
        currentPaths.insert(rel)
        if let existing = storedByPath[rel] {
            if existing.0 != mtime || existing.1 != size {
                changed.append((rel, mtime, size))
            }
        } else {
            changed.append((rel, mtime, size))
        }
    }
    let removedPaths = stored.map { $0.relativePath }.filter { !currentPaths.contains($0) }
    return (changed, removedPaths)
}

/// HI-03: 32 MiB cap matches desktop MAX_FILE_BYTES. Files above this are
/// skipped before any bytes are read into memory so a single large attachment
/// cannot OOM the app.
let MAX_FILE_BYTES: Int64 = 32 * 1024 * 1024

/// Read a file's bytes under the current security scope. Returns nil on failure.
func readFileBytes(rootURL: URL, relativePath: String) -> Data? {
    let fileURL = rootURL.appendingPathComponent(relativePath)
    return try? Data(contentsOf: fileURL)
}

/// Drive one sync pass for a single directory source.
///
/// Caller passes the bookmark BLOB (freshly read from the SQLite row); we resolve it,
/// re-create if stale, enumerate the directory, diff against the stored fingerprints
/// returned by `FfiApp.listDirectoryFingerprints`, read bytes for changed files, and
/// dispatch `AppAction.syncDirectoryFiles` in 50-file chunks with `isFinalBatch: true`
/// on the last one (D-25, T-32-DoS1).
///
/// All steps are parameterised so ContentView / DirectorySourcesView can use the same
/// helper for the initial-sync path and the ScenePhase foreground-resume path.
func syncDirectorySource(
    sourceId: String,
    bookmarkData: Data,
    exclusionGlobs: [String],
    ffiApp: FfiApp,
    dispatch: @escaping (AppAction) -> Void
) -> EnumerationResult {
    // 1) Resolve bookmark + handle staleness.
    let resolved = resolveBookmark(bookmarkData)
    let rootURL: URL
    switch resolved {
    case .ok(let u, let isStale, let refreshed):
        rootURL = u
        if isStale, let fresh = refreshed {
            picker_logger.info("bookmark stale for \(sourceId, privacy: .public) — dispatching update")
            dispatch(.updateDirectorySourceBookmark(sourceId: sourceId, bookmarkData: fresh))
        }
    case .failure(let reason):
        picker_logger.error("bookmark resolve failed for \(sourceId, privacy: .public): \(reason, privacy: .public)")
        return EnumerationResult(entries: [], skippedCloud: [], errors: [reason])
    }

    // 2) Enumerate.
    let enumeration = enumerateDirectory(rootURL: rootURL, exclusionGlobs: exclusionGlobs)
    if !enumeration.errors.isEmpty {
        return enumeration
    }

    // 3) Native-side diff against stored fingerprints (D-02 — keeps 10k-vault perf reasonable).
    let stored: [DirectoryFingerprint]
    do {
        stored = try ffiApp.listDirectoryFingerprints(sourceId: sourceId)
    } catch {
        picker_logger.error("listDirectoryFingerprints failed: \(error.localizedDescription)")
        return EnumerationResult(entries: enumeration.entries,
                                 skippedCloud: enumeration.skippedCloud,
                                 errors: ["listDirectoryFingerprints: \(error.localizedDescription)"])
    }
    let diff = diffAgainstStored(current: enumeration.entries, stored: stored)

    // 4) Read bytes + dispatch in 50-file chunks. Reads must be inside the security scope.
    guard rootURL.startAccessingSecurityScopedResource() else {
        return EnumerationResult(entries: enumeration.entries,
                                 skippedCloud: enumeration.skippedCloud,
                                 errors: ["startAccessingSecurityScopedResource failed for reads"])
    }
    defer { rootURL.stopAccessingSecurityScopedResource() }

    let chunkSize = 50
    let changedChunks = stride(from: 0, to: diff.changed.count, by: chunkSize).map { start -> [(String, Int64, Int64)] in
        Array(diff.changed[start..<min(start + chunkSize, diff.changed.count)])
    }

    if changedChunks.isEmpty {
        // Only removals — send one final batch with empty files list and the removed paths.
        dispatch(.syncDirectoryFiles(
            sourceId: sourceId,
            files: [],
            removedPaths: diff.removedPaths,
            isFinalBatch: true))
    } else {
        for (idx, chunk) in changedChunks.enumerated() {
            let isFinal = idx == changedChunks.count - 1
            var entries: [DirectoryFileEntry] = []
            for (rel, mtime, size) in chunk {
                if size > MAX_FILE_BYTES {
                    picker_logger.warning("skipping oversized file \(rel, privacy: .public) (\(size) bytes > \(MAX_FILE_BYTES) cap)")
                    continue
                }
                guard let bytes = readFileBytes(rootURL: rootURL, relativePath: rel) else {
                    picker_logger.warning("skipping unreadable file \(rel, privacy: .public)")
                    continue
                }
                entries.append(DirectoryFileEntry(
                    relativePath: rel,
                    mtimeSecs: mtime,
                    sizeBytes: size,
                    content: bytes))
            }
            // ME-02: removals ride the FIRST batch (matching desktop + Android
            // conventions) so partial sync interruptions produce consistent
            // cross-platform state instead of iOS=nothing-removed /
            // Android=everything-removed divergence.
            let removedForThisBatch = idx == 0 ? diff.removedPaths : []
            dispatch(.syncDirectoryFiles(
                sourceId: sourceId,
                files: entries,
                removedPaths: removedForThisBatch,
                isFinalBatch: isFinal))
        }
    }

    return enumeration
}
