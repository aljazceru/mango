import Foundation
import Observation
import Security
import os

private let logger = Logger(subsystem: "dev.disobey.mango", category: "AppManager")

// MARK: - KeychainProvider

private class IOSKeychainProvider: KeychainProvider {
    func store(service: String, key: String, value: String) {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key,
        ]
        // Delete existing item first (upsert pattern)
        SecItemDelete(query as CFDictionary)
        // Add new item
        var addQuery = query
        addQuery[kSecValueData as String] = value.data(using: .utf8)!
        SecItemAdd(addQuery as CFDictionary, nil)
    }

    func load(service: String, key: String) -> String? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        guard status == errSecSuccess, let data = result as? Data else { return nil }
        return String(data: data, encoding: .utf8)
    }

    func delete(service: String, key: String) {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key,
        ]
        SecItemDelete(query as CFDictionary)
    }
}

// MARK: - EmbeddingProvider (fallback)

/// Zero-vector fallback used when MobileEmbeddingProvider fails to initialize.
/// Document retrieval runs end-to-end but results are non-semantic --
/// all documents score equally because all embeddings are identical.
/// This class is only active if the ONNX model or tokenizer fails to load.
private class IOSEmbeddingProvider: EmbeddingProvider {
    func embed(texts: [String]) -> [Float] {
        return [Float](repeating: 0.0, count: texts.count * 384)
    }
}

// MARK: - AppManager

@MainActor
@Observable
final class AppManager: AppReconciler, ObservableObject {
    let ffiApp: FfiApp
    var appState: AppState
    var isReady: Bool
    private var lastRevApplied: UInt64
    private var rehydratedDirectorySourceIds = Set<String>()

    init() {
        let fm = FileManager.default
        let dataDirUrl = fm.urls(for: .applicationSupportDirectory, in: .userDomainMask).first!
        let dataDir = dataDirUrl.path
        try? fm.createDirectory(at: dataDirUrl, withIntermediateDirectories: true)

        let keychain = IOSKeychainProvider()
        let embedding: EmbeddingProvider
        let embeddingStatus: EmbeddingStatus
        do {
            embedding = try MobileEmbeddingProvider()
            embeddingStatus = .active
        } catch {
            logger.warning("MobileEmbeddingProvider init failed: \(error.localizedDescription), falling back to null")
            embedding = IOSEmbeddingProvider()
            embeddingStatus = .degraded
        }
        let localLlm = IOSLocalLlmProvider()
        let biometric = BiometricProviderImpl()
        let app = FfiApp(
            dataDir: dataDir,
            keychain: keychain,
            embeddingProvider: embedding,
            embeddingStatus: embeddingStatus,
            localLlmProvider: localLlm,
            biometricProvider: biometric
        )
        self.ffiApp = app

        let initial = app.state()
        self.appState = initial
        self.isReady = false
        self.lastRevApplied = initial.rev

        app.listenForUpdates(reconciler: self)
        rehydrateDirectoryBookmarks(from: initial)
    }

    nonisolated func reconcile(update: AppUpdate) {
        Task { @MainActor [weak self] in
            self?.apply(update: update)
        }
    }

    private func apply(update: AppUpdate) {
        switch update {
        case .fullState(let s):
            if s.rev < lastRevApplied { return }
            lastRevApplied = s.rev
            appState = s
            isReady = true
            rehydrateDirectoryBookmarks(from: s)
        }
    }

    func dispatch(_ action: AppAction) {
        ffiApp.dispatch(action: action)
    }

    private func rehydrateDirectoryBookmarks(from state: AppState) {
        // Rehydrate DirectorySyncScheduler.bookmarkCache from persisted
        // directory_sources.bookmark_data so foreground sync can run after cold
        // launch even when the first actor-emitted state arrives with the same
        // rev as the constructor snapshot.
        for source in state.directorySources {
            guard !rehydratedDirectorySourceIds.contains(source.id) else { continue }
            do {
                if let blob = try ffiApp.getDirectoryBookmark(sourceId: source.id) {
                    DirectorySyncScheduler.cacheBookmark(sourceId: source.id, bookmarkData: blob)
                } else {
                    // Desktop/Android-shaped row (no bookmark) is expected.
                    logger.info("AppManager rehydrate: no bookmark for source \(source.id, privacy: .public)")
                }
                rehydratedDirectorySourceIds.insert(source.id)
            } catch {
                logger.warning("AppManager rehydrate: getDirectoryBookmark failed for \(source.id, privacy: .public): \(error.localizedDescription)")
            }
        }
    }
}
