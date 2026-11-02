package dev.disobey.mango.ui

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.DocumentsContract
import android.provider.MediaStore
import android.util.Log
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.annotation.ChecksSdkIntAtLeast
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

/**
 * Bitmask of URI-permission flags persisted in the picker callback.
 * MUST include BOTH read and write — see
 * .planning/debug/resolved/android-saf-cant-use-folder-grapheneos.md (Evidence #2):
 * OpenDocumentTree grants both by default and some providers (GrapheneOS) require
 * write to be acknowledged even for read-only consumers.
 */
internal const val PERSISTABLE_URI_FLAGS: Int =
    Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION

/**
 * Returns true when the SDK version supports `MediaStore.Downloads` (API 29+) and the
 * picker should be seeded with the Downloads URI to avoid the Android 11+ internal-
 * storage root restriction.
 *
 * Exposed as a pure-Kotlin predicate so JVM unit tests can assert the SDK-gating
 * logic without touching `android.net.Uri` (whose constructor is package-private and
 * factory methods throw in the stubbed test android.jar).
 *
 * See .planning/debug/resolved/android-saf-cant-use-folder-grapheneos.md for the full
 * root-cause trace. Reverting this to always return false is a regression — covered by
 * DirectorySourcePickerTest.
 *
 * `@ChecksSdkIntAtLeast` makes the SDK gate visible to Android lint: every production
 * caller passes `Build.VERSION.SDK_INT`, so a true return implies API 29+ and lint
 * accepts the `MediaStore.Downloads` access in `initialTreeUriForSdk` (previously
 * NewApi: the plain Boolean helper was not recognized as an SDK-int guard).
 */
@ChecksSdkIntAtLeast(api = Build.VERSION_CODES.Q)
internal fun useDownloadsUriForSdk(sdkInt: Int): Boolean = sdkInt >= Build.VERSION_CODES.Q

/**
 * Initial URI passed to ActivityResultContracts.OpenDocumentTree launch().
 *
 * Returning null causes DocumentsUI to open at the internal-storage root, which on
 * Android 11+ (API 30+) triggers the "Can't use this folder" privacy banner for
 * every subfolder navigated to from there. Seeding with Downloads side-steps the
 * restriction because the user starts inside a non-root subtree.
 *
 * See .planning/debug/resolved/android-saf-cant-use-folder-grapheneos.md for the
 * full root-cause trace. Reverting this to `null` is a regression — covered by
 * DirectorySourcePickerTest.
 */
internal fun initialTreeUriForSdk(sdkInt: Int): Uri? =
    if (useDownloadsUriForSdk(sdkInt)) MediaStore.Downloads.EXTERNAL_CONTENT_URI else null

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
 *
 * Bug fix (GrapheneOS / Android 11+): We pass a non-restricted initial URI to the
 * picker so DocumentsUI opens inside a permitted subtree rather than the internal
 * storage root. When the picker opens at the root ("Pixel 9a" in GrapheneOS
 * DocumentsUI), Android 11+ (API 30+) blocks selecting *any* folder that the user
 * reaches by navigating down from that root, showing "Can't use this folder / To
 * protect your privacy, choose another folder". Seeding with
 * `MediaStore.Downloads.EXTERNAL_CONTENT_URI` opens the picker inside Downloads,
 * which is an unrestricted starting point — the user can still navigate anywhere
 * from there. Fallback to null on API < 29 (Downloads MediaStore URI requires 29+).
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
        // Take both read AND write flags — OpenDocumentTree grants both, and some
        // ContentResolver implementations (including GrapheneOS) require the write
        // flag to be acknowledged even for read-only consumers.
        try {
            context.contentResolver.takePersistableUriPermission(uri, PERSISTABLE_URI_FLAGS)
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
    // Seed the picker at the public Downloads directory (API 29+). This avoids
    // opening at the internal-storage root which triggers Android 11+'s
    // "Can't use this folder" privacy restriction for all subfolders navigated to
    // from that root. On API < 29 fall back to null (no initial URI hint).
    return { launcher.launch(initialTreeUriForSdk(Build.VERSION.SDK_INT)) }
}

/**
 * A tree query failed or returned a null cursor: the enumeration is incomplete,
 * so treating it as truth would delete valid indexed documents. Fail the pass.
 */
class DirectoryEnumerationException(message: String) : Exception(message)

/**
 * Bulk SAF traversal using `DocumentsContract.buildChildDocumentsUriUsingTree` +
 * `ContentResolver.query`. **NEVER** use `DocumentFile.listFiles()` here — that
 * path issues one IPC per child and is 10-100x slower on large vaults
 * (Pitfall 5 in 32-RESEARCH.md).
 *
 * A BFS stack is used with one bulk query per directory level. Any failed or
 * null query throws [DirectoryEnumerationException] — a partial enumeration
 * must never be diffed as file deletions.
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
        val cursor = try {
            context.contentResolver.query(childrenUri, projection, null, null, null)
        } catch (se: SecurityException) {
            throw DirectoryEnumerationException("permission lost for $parentDocId: ${se.message}")
        } catch (e: Exception) {
            throw DirectoryEnumerationException("query failed for $parentDocId: ${e.message}")
        } ?: throw DirectoryEnumerationException("null cursor for $parentDocId")
        cursor.use {
            while (it.moveToNext()) {
                val docId = it.getString(0) ?: continue
                val name = it.getString(1) ?: continue
                val mtime = it.getLong(2) // millis (D-20)
                val size = it.getLong(3)
                val mime = it.getString(4) ?: ""
                val isDir = mime == DocumentsContract.Document.MIME_TYPE_DIR
                if (matcher.matches(name)) continue
                if (isDir) {
                    stack.addLast(docId)
                } else {
                    results.add(SafChildEntry(docId, name, mtime, size, false))
                }
            }
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

/**
 * Read up to [maxBytes]+1 bytes from a stream; returns null when the stream is
 * longer than [maxBytes]. Bounds memory even when the provider lies about the
 * declared file size or the file grows mid-read.
 */
fun readBytesCapped(input: java.io.InputStream, maxBytes: Long): ByteArray? {
    require(maxBytes in 0..Int.MAX_VALUE.toLong())
    val out = java.io.ByteArrayOutputStream()
    val buf = ByteArray(8192)
    while (out.size() <= maxBytes) {
        val n = input.read(buf, 0, minOf(buf.size.toLong(), maxBytes - out.size() + 1).toInt())
        if (n < 0) break
        out.write(buf, 0, n)
    }
    return if (out.size() > maxBytes) null else out.toByteArray()
}

/** Read a single file's bytes via a document URI rooted in the persisted tree.
 * The cap is enforced on the ACTUAL stream, not the advertised size. */
fun readFileContent(context: Context, treeUri: Uri, docId: String): ByteArray? {
    val fileUri = DocumentsContract.buildDocumentUriUsingTree(treeUri, docId)
    return try {
        context.contentResolver.openInputStream(fileUri)?.use {
            readBytesCapped(it, MAX_FILE_BYTES)
        }
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

    // Enumeration must complete before ANY diff runs — a partial listing
    // (revoked permission, dead provider, null cursor) must never be treated
    // as file deletions. On failure the previous index stays untouched.
    val children = try {
        traverseTree(context, treeUri, source.exclusionGlobs)
    } catch (e: DirectoryEnumerationException) {
        Log.e("DirectorySourcePicker", "enumeration failed for ${source.id}: ${e.message}")
        dispatch(
            AppAction.ShowToast(
                message = "Could not read \"${source.displayName}\" — sync skipped, index kept.",
            )
        )
        return@withContext
    }

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

    fun finalBatch(removed: List<String>) = dispatch(
        AppAction.SyncDirectoryFiles(
            sourceId = source.id,
            files = emptyList(),
            removedPaths = removed,
            isFinalBatch = true,
        )
    )

    if (changed.isEmpty() && removedPaths.isEmpty()) {
        // Nothing to do — fire a single no-op final batch so the actor can update
        // last_synced_at and flip status back to Idle.
        finalBatch(emptyList())
        return@withContext
    }

    // Files are read one at a time and batches are bounded by BYTES (not just
    // count) so a directory of max-size files cannot OOM the app; the next
    // batch is only materialized after the actor has processed the previous
    // one (syncActor is a FIFO barrier over dispatched actions).
    var batch = mutableListOf<DirectoryFileEntry>()
    var batchBytes = 0L
    var batchIndex = 0

    fun flushBatch(lastBatch: Boolean) {
        // Only the FIRST batch carries `removedPaths` so the cascade fires
        // once (matches the iOS pipeline in 32-05 DirectorySourcePicker.swift).
        val removedForThisBatch = if (batchIndex == 0) removedPaths else emptyList()
        dispatch(
            AppAction.SyncDirectoryFiles(
                sourceId = source.id,
                files = batch.toList(),
                removedPaths = removedForThisBatch,
                isFinalBatch = lastBatch,
            )
        )
        batchIndex += 1
        batch = mutableListOf()
        batchBytes = 0L
        if (!lastBatch) manager.syncActor()
    }

    for (e in changed) {
        if (e.sizeBytes > MAX_FILE_BYTES) {
            Log.w(
                "DirectorySourcePicker",
                "skipping oversized file ${e.name} (${e.sizeBytes} bytes > $MAX_FILE_BYTES cap)",
            )
            continue
        }
        if (batchBytes + MAX_FILE_BYTES > MAX_SYNC_BATCH_BYTES || batch.size >= 50) {
            flushBatch(lastBatch = false)
        }
        val bytes = readFileContent(context, treeUri, e.docId)
        if (bytes == null) {
            Log.w("DirectorySourcePicker", "skipping unreadable/oversized stream for ${e.name}")
            continue
        }
        batch += DirectoryFileEntry(
            relativePath = e.docId,
            mtimeSecs = e.mtimeSecs,
            sizeBytes = e.sizeBytes,
            content = bytes,
        )
        batchBytes += bytes.size
    }

    // Last batch: pending files (or an empty final batch carrying removals,
    // including when nothing changed but files were deleted).
    flushBatch(lastBatch = true)
}

/** Per-batch byte ceiling: bounds retained file content in flight to the actor. */
private const val MAX_SYNC_BATCH_BYTES: Long = 64L * 1024L * 1024L
