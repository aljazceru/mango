package dev.disobey.mango.ui

import android.content.Context
import android.net.Uri
import android.provider.DocumentsContract
import android.util.Log
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import dev.disobey.mango.AppManager
import dev.disobey.mango.rust.DirectorySourceSummary
import java.util.concurrent.TimeUnit

/**
 * Phase 32 Plan 06 — periodic directory-sync worker.
 *
 * Runs every 15 minutes (the WorkManager platform minimum, D-23) and re-syncs
 * every directory source registered in AppState. Uses the shared
 * `syncDirectory` pipeline from `DirectorySourcePicker.kt` so behaviour is
 * identical to foreground onResume-triggered syncs (zero pipeline divergence).
 *
 * No network constraint: SAF reads are local IPC to the storage provider and
 * never touch the radio (avoids unnecessary wake — T-32-DoS5 mitigation).
 */
class DirectorySyncWorker(
    appContext: Context,
    params: WorkerParameters,
) : CoroutineWorker(appContext, params) {
    override suspend fun doWork(): Result {
        val manager = AppManager.getInstance(applicationContext)
        val sources = manager.state.directorySources
        for (source in sources) {
            val tree = resolveTreeUri(applicationContext, source) ?: continue
            try {
                syncDirectory(applicationContext, source, Uri.parse(tree)) { action ->
                    manager.dispatch(action)
                }
            } catch (t: Throwable) {
                Log.e(TAG, "sync failed for ${source.id}: ${t.message}")
                // Continue to the next source — one bad source must not poison the run.
            }
        }
        return Result.success()
    }

    companion object {
        private const val TAG = "DirectorySyncWorker"
        const val UNIQUE_NAME = "directory_sync"

        /**
         * Enqueue the 15-minute periodic sync. Uses `KEEP` so re-enqueues (e.g.
         * in `onCreate` after a config change) are no-ops when the schedule
         * already exists.
         */
        fun enqueue(context: Context) {
            val req = PeriodicWorkRequestBuilder<DirectorySyncWorker>(15, TimeUnit.MINUTES).build()
            WorkManager.getInstance(context).enqueueUniquePeriodicWork(
                UNIQUE_NAME,
                ExistingPeriodicWorkPolicy.KEEP,
                req,
            )
        }
    }
}

/**
 * DirectorySourceSummary does NOT carry the platform-specific handles across the
 * UniFFI boundary (T-32-I2). On Android the tree URI is recovered from the
 * ContentResolver's persistable-permission list, which survives process death
 * and device reboot (Pitfall 4 / D-18).
 *
 * Matching strategy: derive the display name from each persisted tree URI using
 * the same rule the picker used, and match against the source's displayName.
 * This keeps the URI inside the app without ever crossing UniFFI (T-32-I2).
 */
internal fun resolveTreeUri(
    context: Context,
    source: DirectorySourceSummary,
): String? {
    val persisted = try {
        context.contentResolver.persistedUriPermissions
    } catch (_: SecurityException) {
        return null
    }
    for (grant in persisted) {
        if (!grant.isReadPermission) continue
        val uri = grant.uri
        val name = try {
            DocumentsContract.getTreeDocumentId(uri)
                .substringAfterLast(':')
                .substringAfterLast('/')
                .ifEmpty { "Folder" }
        } catch (_: Exception) {
            continue
        }
        if (name == source.displayName) return uri.toString()
    }
    return null
}
