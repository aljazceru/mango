package dev.disobey.mango.ui

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.DocumentsContract
import android.util.Log
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.Composable
import dev.disobey.mango.AppManager
import dev.disobey.mango.rust.AppAction
import dev.disobey.mango.rust.DirectoryFileEntry
import dev.disobey.mango.rust.DirectoryFingerprint
import dev.disobey.mango.rust.DirectorySourceSummary
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * Phase 32 Plan 06 — Android SAF directory-sync support.
 *
 * Provides a persistable-permission tree-URI picker and a bulk `DocumentsContract`
 * traversal that MUST be used in place of `DocumentFile.listFiles()` (see Pitfall 5
 * in 32-RESEARCH.md — 10-100x IPC overhead on large vaults).
 *
 * The bookmark-equivalent on Android is the persisted tree URI; we call
 * `takePersistableUriPermission` immediately in the picker callback so the grant
 * survives process death and device reboot (D-18 / Pitfall 4).
 */

/** A single SAF document entry produced by `traverseTree`. */
data class SafChildEntry(
    val docId: String,
    val name: String,
    val lastModifiedMs: Long,
    val sizeBytes: Long,
    val isDirectory: Boolean,
)

/**
 * Remember a SAF folder-picker launcher that takes persistable read permission on pick.
 *
 * On success the URI + a derived displayName are handed to `onPicked`.
 * Pitfall 4: `takePersistableUriPermission` MUST be called before the URI is stored
 * or released; we do it here inline.
 */
@Composable
fun rememberDirectoryPicker(onPicked: (Uri, String) -> Unit): () -> Unit {
    val context = androidx.compose.ui.platform.LocalContext.current
    val launcher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenDocumentTree()
    ) { uri: Uri? ->
        uri ?: return@rememberLauncherForActivityResult
        // D-18 / Pitfall 4: MUST take persistable permission before returning so
        // the grant survives process death / device reboot.
        val flags = Intent.FLAG_GRANT_READ_URI_PERMISSION
        try {
            context.contentResolver.takePersistableUriPermission(uri, flags)
        } catch (se: SecurityException) {
            Log.e("DirectorySourcePicker", "takePersistableUriPermission failed: ${se.message}")
            return@rememberLauncherForActivityResult
        }
        val displayName = try {
            DocumentsContract.getTreeDocumentId(uri)
                .substringAfterLast(':')
                .substringAfterLast('/')
                .ifEmpty { "Folder" }
        } catch (_: Exception) {
            "Folder"
        }
        onPicked(uri, displayName)
    }
    return { launcher.launch(null) }
}

/**
 * Bulk SAF traversal using `DocumentsContract.buildChildDocumentsUriUsingTree` +
 * `ContentResolver.query`. **NEVER** use `DocumentFile.listFiles()` here — that
 * path issues one IPC per child and is 10-100x slower on large vaults
 * (Pitfall 5 in 32-RESEARCH.md).
 *
 * A BFS stack is used with one bulk query per directory level.
 */
fun traverseTree(
    context: Context,
    treeUri: Uri,
    exclusionGlobs: List<String>,
): List<SafChildEntry> {
    val results = mutableListOf<SafChildEntry>()
    val stack = ArrayDeque<String>()
    stack.addLast(DocumentsContract.getTreeDocumentId(treeUri))
    val matcher = GlobMatcher(exclusionGlobs)
    val projection = arrayOf(
        DocumentsContract.Document.COLUMN_DOCUMENT_ID,
        DocumentsContract.Document.COLUMN_DISPLAY_NAME,
        DocumentsContract.Document.COLUMN_LAST_MODIFIED,
        DocumentsContract.Document.COLUMN_SIZE,
        DocumentsContract.Document.COLUMN_MIME_TYPE,
    )
    while (stack.isNotEmpty()) {
        val parentDocId = stack.removeLast()
        val childrenUri = DocumentsContract.buildChildDocumentsUriUsingTree(treeUri, parentDocId)
        try {
            context.contentResolver.query(childrenUri, projection, null, null, null)?.use { cursor ->
                while (cursor.moveToNext()) {
                    val docId = cursor.getString(0) ?: continue
                    val name = cursor.getString(1) ?: continue
                    val mtime = cursor.getLong(2) // millis (D-20)
                    val size = cursor.getLong(3)
                    val mime = cursor.getString(4) ?: ""
                    val isDir = mime == DocumentsContract.Document.MIME_TYPE_DIR
                    if (matcher.matches(name)) continue
                    if (isDir) {
                        stack.addLast(docId)
                    } else {
                        results.add(SafChildEntry(docId, name, mtime, size, false))
                    }
                }
            }
        } catch (se: SecurityException) {
            Log.e("DirectorySourcePicker", "query failed for $parentDocId: ${se.message}")
            // Continue traversal of other branches; don't abort.
        } catch (e: Exception) {
            Log.e("DirectorySourcePicker", "query error for $parentDocId: ${e.message}")
        }
    }
    return results
}

/**
 * HI-03: 32 MiB cap matches desktop MAX_FILE_BYTES / iOS MAX_FILE_BYTES.
 * Files above this are skipped before `readBytes()` is called so a single
 * large attachment inside a vault cannot OOM the app.
 */
const val MAX_FILE_BYTES: Long = 32L * 1024L * 1024L

/** Read a single file's bytes via a document URI rooted in the persisted tree. */
fun readFileContent(context: Context, treeUri: Uri, docId: String): ByteArray? {
    val fileUri = DocumentsContract.buildDocumentUriUsingTree(treeUri, docId)
    return try {
        context.contentResolver.openInputStream(fileUri)?.use { it.readBytes() }
    } catch (_: SecurityException) {
        null
    } catch (_: Exception) {
        null
    }
}

/**
 * Minimal glob matcher covering the Obsidian-default preset set:
 *  - directory prefixes (`.obsidian/`)
 *  - extension globs (`*.tmp`, `*.canvas`)
 *  - literal names (`.DS_Store`)
 *
 * Exhaustive validation lives on the Rust side (AddDirectorySource /
 * SetDirectoryExclusions both run `validate_glob_pattern` per D-29); this matcher
 * is only used for on-the-fly traversal skipping.
 */
class GlobMatcher(patterns: List<String>) {
    private val prefixes = patterns.filter { it.endsWith("/") }.map { it.trimEnd('/') }
    private val extensions = patterns.filter { it.startsWith("*.") }.map { it.removePrefix("*") }
    private val literals = patterns.filter { !it.endsWith("/") && !it.startsWith("*.") }

    fun matches(name: String): Boolean =
        prefixes.any { name == it || name.startsWith("$it/") } ||
            extensions.any { name.endsWith(it) } ||
            literals.any { name == it }
}

/**
 * Default exclusion presets offered on source creation.
 *
 * Mirrors the iOS plan 05 / desktop plan 04 preset lists (Obsidian workflow).
 */
val DEFAULT_EXCLUSION_PRESETS: List<String> = listOf(
    ".obsidian/",
    ".trash/",
    "*.tmp",
    "*.canvas",
    ".git/",
)

/**
 * End-to-end sync for a single directory source:
 *   traverse (bulk) → native diff vs `listDirectoryFingerprints` →
 *   batched (50) `SyncDirectoryFiles` dispatches.
 *
 * Runs entirely off the UI thread (callers should invoke from a coroutine on
 * Dispatchers.IO or inside a CoroutineWorker).
 *
 * `dispatch` is the action-dispatcher (usually `AppManager.dispatch`).
 */
suspend fun syncDirectory(
    context: Context,
    source: DirectorySourceSummary,
    treeUri: Uri,
    dispatch: (AppAction) -> Unit,
) = withContext(Dispatchers.IO) {
    val manager = AppManager.getInstance(context.applicationContext)
    val children = traverseTree(context, treeUri, source.exclusionGlobs)

    // Build (docId, mtime_secs, size) entries. D-08: if mtime == 0 substitute
    // `now` so the diff treats the file as always-modified (fallback signal).
    data class LocalEntry(
        val docId: String,
        val name: String,
        val mtimeSecs: Long,
        val sizeBytes: Long,
    )
    val nowSecs = System.currentTimeMillis() / 1000 // D-20: ms/1000 → secs
    val localEntries: List<LocalEntry> = children.map { child ->
        val mtimeSecs = if (child.lastModifiedMs == 0L) {
            nowSecs // D-08 fallback — always treated as modified
        } else {
            child.lastModifiedMs / 1000 // D-20: divide by 1000
        }
        LocalEntry(child.docId, child.name, mtimeSecs, child.sizeBytes)
    }

    // Fetch stored fingerprints from the actor for native-side diff (D-02).
    // Mirrors the iOS 32-05 / desktop 32-04 pattern.
    val stored: List<DirectoryFingerprint> = try {
        manager.listDirectoryFingerprints(source.id)
    } catch (e: Exception) {
        Log.e("DirectorySourcePicker", "listDirectoryFingerprints(${source.id}) failed: ${e.message}")
        return@withContext
    }
    val storedByPath: Map<String, DirectoryFingerprint> =
        stored.associateBy { it.relativePath }

    val currentIds: Set<String> = localEntries.map { it.docId }.toSet()
    val removedPaths: List<String> = stored.map { it.relativePath }.filter { it !in currentIds }

    val changed: List<LocalEntry> = localEntries.filter { e ->
        val prev = storedByPath[e.docId] ?: return@filter true // added
        // modified iff mtime or size changed (matches diff_files semantics)
        prev.mtimeSecs != e.mtimeSecs || prev.sizeBytes != e.sizeBytes
    }

    if (changed.isEmpty() && removedPaths.isEmpty()) {
        // Nothing to do — fire a single no-op final batch so the actor can update
        // last_synced_at and flip status back to Idle.
        dispatch(
            AppAction.SyncDirectoryFiles(
                sourceId = source.id,
                files = emptyList(),
                removedPaths = emptyList(),
                isFinalBatch = true,
            )
        )
        return@withContext
    }

    // Chunk changed files into 50-entry batches (T-32-DoS1 — matches the Rust-side
    // ceiling as defence-in-depth).
    val chunks = changed.chunked(50)
    chunks.forEachIndexed { idx, chunk ->
        val entries: List<DirectoryFileEntry> = chunk.mapNotNull { e ->
            if (e.sizeBytes > MAX_FILE_BYTES) {
                Log.w(
                    "DirectorySourcePicker",
                    "skipping oversized file ${e.name} (${e.sizeBytes} bytes > $MAX_FILE_BYTES cap)",
                )
                return@mapNotNull null
            }
            val bytes = readFileContent(context, treeUri, e.docId) ?: return@mapNotNull null
            DirectoryFileEntry(
                relativePath = e.docId,
                mtimeSecs = e.mtimeSecs,
                sizeBytes = e.sizeBytes,
                content = bytes,
            )
        }
        val isFinal = idx == chunks.lastIndex
        // Only the FIRST batch carries the `removedPaths` list so the cascade
        // fires once; subsequent batches pass an empty list. Matches the iOS
        // pipeline in 32-05 DirectorySourcePicker.swift.
        val removedForThisBatch = if (idx == 0) removedPaths else emptyList()
        dispatch(
            AppAction.SyncDirectoryFiles(
                sourceId = source.id,
                files = entries,
                removedPaths = removedForThisBatch,
                isFinalBatch = isFinal,
            )
        )
    }

    // If we had removed paths but no changed files at all, still send a final
    // batch with the removals.
    if (chunks.isEmpty() && removedPaths.isNotEmpty()) {
        dispatch(
            AppAction.SyncDirectoryFiles(
                sourceId = source.id,
                files = emptyList(),
                removedPaths = removedPaths,
                isFinalBatch = true,
            )
        )
    }
}
