import SwiftUI
import UniformTypeIdentifiers

/// Document Library screen: manages local document collection for RAG (LRAG-06, D-09, D-10).
/// Allows adding documents via file importer, viewing library contents, and deleting documents.
struct DocumentLibraryView: View {
    @EnvironmentObject var appManager: AppManager

    @State private var showFileImporter = false
    @State private var showDocAttachSheet = false

    var appState: AppState { appManager.appState }

    // MARK: - Body

    var body: some View {
        NavigationStack {
            Group {
                if appState.documents.isEmpty && appState.ingestionProgress == nil {
                    emptyStateView
                } else {
                    documentListView
                }
            }
            .navigationTitle("Document Library")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Back") {
                        appManager.dispatch(.popScreen)
                    }
                }
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Add Document") {
                        showFileImporter = true
                    }
                    .font(.subheadline)
                    .accessibilityLabel("Add a document to the library")
                }
            }
            .fileImporter(
                isPresented: $showFileImporter,
                allowedContentTypes: [.pdf, .plainText, .text],
                allowsMultipleSelection: false
            ) { result in
                handleFileImportResult(result)
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
            Text("No documents yet")
                .font(.headline)
                .foregroundStyle(.primary)
            Text("Tap \"Add Document\" to ingest a PDF, text, or Markdown file.")
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)
            Spacer()
        }
    }

    private var documentListView: some View {
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

            // Document list
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

    private func documentRow(_ doc: DocumentSummary) -> some View {
        HStack(spacing: 12) {
            Image(systemName: formatIcon(doc.format))
                .foregroundStyle(.accentColor)
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

    // MARK: - File Import

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
                        content: Array(data)
                    ))
                } catch {
                    // File read error -- swallow silently; future plan adds toast
                }
            }
        case .failure:
            break
        }
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
