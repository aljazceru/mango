import SwiftUI
import UniformTypeIdentifiers

/// Unified RAG screen: lists documents + directory sources together (LRAG-06, DIR-05).
/// The Home toolbar routes here via `.documents`; the legacy DirectorySourcesView
/// remains reachable by tapping a folder row.
struct DocumentLibraryView: View {
    @EnvironmentObject var appManager: AppManager

    @State private var showFileImporter = false
    @State private var showFolderPicker = false
    /// Bookmark cache keyed by displayName (pre-id fallback) then promoted to
    /// sourceId once the new row appears in AppState. Mirrors DirectorySourcesView.
    @State private var bookmarkCache: [String: Data] = [:]

    /// Default exclusion preset (Obsidian-friendly) offered for new sources (D-29).
    private let defaultExclusions: [String] = [
        ".obsidian/", ".trash/", "*.tmp", "*.canvas", ".git/",
    ]

    var appState: AppState { appManager.appState }

    // MARK: - Body

    var body: some View {
        NavigationStack {
            Group {
                if appState.documents.isEmpty
                    && appState.directorySources.isEmpty
                    && appState.ingestionProgress == nil
                {
                    emptyStateView
                } else {
                    unifiedListView
                }
            }
            .navigationTitle("RAG")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Back") {
                        appManager.dispatch(.popScreen)
                    }
                }
                ToolbarItem(placement: .navigationBarTrailing) {
                    Menu {
                        Button {
                            showFileImporter = true
                        } label: {
                            Label("Document", systemImage: "doc")
                        }
                        Button {
                            showFolderPicker = true
                        } label: {
                            Label("Folder", systemImage: "folder")
                        }
                    } label: {
                        Image(systemName: "plus")
                            .font(.subheadline)
                    }
                    .accessibilityLabel("Add a RAG source")
                }
            }
            .fileImporter(
                isPresented: $showFileImporter,
                allowedContentTypes: [.pdf, .plainText, .text],
                allowsMultipleSelection: false
            ) { result in
                handleFileImportResult(result)
            }
            .sheet(isPresented: $showFolderPicker) {
                DirectorySourcePicker(
                    onPicked: { url, bookmarkData in
                        handlePickedFolder(url: url, bookmarkData: bookmarkData)
                        showFolderPicker = false
                    },
                    onCancel: { showFolderPicker = false })
            }
        }
    }

    // MARK: - Subviews

    private var emptyStateView: some View {
        VStack(spacing: 12) {
            Spacer()
            Image(systemName: "doc.text")
                .font(.system(size: 48))
                .foregroundStyle(.secondary)
            Text("No RAG sources yet")
                .font(.headline)
                .foregroundStyle(.primary)
            Text("Tap + to add a document or a folder.")
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)
            Spacer()
        }
    }

    private var unifiedListView: some View {
        List {
            // Ingestion progress indicator
            if let progress = appState.ingestionProgress {
                Section {
                    HStack(spacing: 12) {
                        ProgressView()
                            .progressViewStyle(.circular)
                            .scaleEffect(0.8)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(progress.documentName)
                                .font(.subheadline)
                                .lineLimit(1)
                            Text(progress.stage + "...")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                    }
                    .padding(.vertical, 4)
                } header: {
                    Text("Ingesting")
                }
            }

            // Folders section — shown only when non-empty.
            if !appState.directorySources.isEmpty {
                Section("Folders") {
                    ForEach(appState.directorySources, id: \.id) { src in
                        directorySourceCompactRow(src)
                    }
                }
            }

            // Documents section — shown only when non-empty.
            if !appState.documents.isEmpty {
                Section("Documents") {
                    ForEach(appState.documents, id: \.id) { doc in
                        documentRow(doc)
                    }
                    .onDelete { indexSet in
                        for index in indexSet {
                            let doc = appState.documents[index]
                            appManager.dispatch(.deleteDocument(documentId: doc.id))
                        }
                    }
                }
            }
        }
        .listStyle(.insetGrouped)
    }

    /// Compact folder row inside the unified RAG list. Tapping pushes
    /// Screen.directorySources so the full management UI (exclusions, sync,
    /// remove) stays reachable without being a top-level Home entry.
    private func directorySourceCompactRow(_ src: DirectorySourceSummary) -> some View {
        Button {
            appManager.dispatch(.pushScreen(screen: .directorySources))
        } label: {
                HStack(spacing: 12) {
                    Image(systemName: "folder.fill")
                    .foregroundStyle(Color.accentColor)
                    .font(.title3)
                    .frame(width: 28)

                VStack(alignment: .leading, spacing: 2) {
                    Text(src.displayName)
                        .font(.subheadline)
                        .foregroundStyle(.primary)
                        .lineLimit(1)
                    HStack(spacing: 6) {
                        compactStatusLabel(src)
                        Text("\(src.fileCount) files")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        Text("·")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        Text(src.lastSyncedLabel)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }

                Spacer()

                Image(systemName: "chevron.right")
                    .font(.caption)
                    .foregroundStyle(.tertiary)
            }
            .padding(.vertical, 2)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    @ViewBuilder
    private func compactStatusLabel(_ src: DirectorySourceSummary) -> some View {
        switch src.syncStatus {
        case .idle:
            EmptyView()
        case .syncing:
            HStack(spacing: 4) {
                ProgressView().scaleEffect(0.5)
                Text("Syncing")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        case .error(let message):
            Text("Error: \(message)")
                .font(.caption)
                .foregroundStyle(.red)
                .lineLimit(1)
        }
    }

    private func documentRow(_ doc: DocumentSummary) -> some View {
        HStack(spacing: 12) {
            Image(systemName: formatIcon(doc.format))
                .foregroundStyle(Color.accentColor)
                .font(.title3)
                .frame(width: 28)

            VStack(alignment: .leading, spacing: 2) {
                Text(doc.name)
                    .font(.subheadline)
                    .lineLimit(1)
                HStack(spacing: 6) {
                    Text(formatBadge(doc.format))
                        .font(.caption2)
                        .padding(.horizontal, 5)
                        .padding(.vertical, 1)
                        .background(Color.accentColor.opacity(0.15))
                        .clipShape(RoundedRectangle(cornerRadius: 4))
                    Text(formatSize(doc.sizeBytes))
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    Text("·")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    Text(formatDate(doc.ingestionDate))
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    Text("·")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    Text("\(doc.chunkCount) chunks")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }

            Spacer()

            Button(role: .destructive) {
                appManager.dispatch(.deleteDocument(documentId: doc.id))
            } label: {
                Image(systemName: "trash")
                    .font(.subheadline)
            }
            .buttonStyle(.borderless)
        }
        .padding(.vertical, 2)
    }

    // MARK: - File / Folder Import

    private func handleFileImportResult(_ result: Result<[URL], Error>) {
        switch result {
        case .success(let urls):
            guard let url = urls.first else { return }
            Task {
                guard url.startAccessingSecurityScopedResource() else { return }
                defer { url.stopAccessingSecurityScopedResource() }
                do {
                    let data = try Data(contentsOf: url)
                    let filename = url.lastPathComponent
                    appManager.dispatch(.ingestDocument(
                        filename: filename,
                        content: data
                    ))
                } catch {
                    // File read error -- swallow silently; future plan adds toast
                }
            }
        case .failure:
            break
        }
    }

    /// Folder picker handler — matches DirectorySourcesView.handlePicked behavior.
    /// The security-scoped bookmark BLOB is persisted via AddDirectorySource so
    /// cold-launch rehydration (Phase 32 Plan 08) continues to work.
    private func handlePickedFolder(url: URL, bookmarkData: Data) {
        let displayName = url.lastPathComponent
        appManager.dispatch(.addDirectorySource(
            displayName: displayName,
            path: nil,
            bookmarkData: bookmarkData,
            treeUri: nil,
            exclusionGlobs: defaultExclusions))
        // Cache under displayName so subsequent Sync Now on the DirectorySources
        // detail screen can resolve the bookmark immediately. Promotion to the
        // real sourceId happens inside DirectorySourcesView (same pattern as the
        // dedicated screen's handlePicked). We also register with the process-wide
        // scheduler so ScenePhase foreground-resume can find the bookmark.
        bookmarkCache[displayName] = bookmarkData
        DirectorySyncScheduler.cacheBookmark(sourceId: displayName, bookmarkData: bookmarkData)
    }

    // MARK: - Helpers

    private func formatIcon(_ format: String) -> String {
        switch format {
        case "pdf": return "doc.fill"
        default: return "doc.text.fill"
        }
    }

    private func formatBadge(_ format: String) -> String {
        switch format {
        case "pdf": return "PDF"
        case "md": return "MD"
        default: return "TXT"
        }
    }

    private func formatSize(_ bytes: UInt64) -> String {
        if bytes < 1024 {
            return "\(bytes) B"
        } else if bytes < 1024 * 1024 {
            return String(format: "%.1f KB", Double(bytes) / 1024.0)
        } else {
            return String(format: "%.1f MB", Double(bytes) / (1024.0 * 1024.0))
        }
    }

    private func formatDate(_ unixTimestamp: Int64) -> String {
        let now = Int64(Date().timeIntervalSince1970)
        let diff = now - unixTimestamp
        if diff < 60 { return "just now" }
        if diff < 3600 { return "\(diff / 60)m ago" }
        if diff < 86400 { return "\(diff / 3600)h ago" }
        let days = diff / 86400
        if days == 1 { return "yesterday" }
        return "\(days)d ago"
    }
}
