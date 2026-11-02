package dev.disobey.mango.ui

import android.content.Context
import android.net.Uri
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
            val tree = resolveTreeUri(manager, source) ?: continue
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
 * UniFFI boundary (T-32-I2). The exact tree URI stored per source id is fetched
 * through `FfiApp.getDirectoryTreeUri` — display-name matching is gone, so two
 * folders with the same basename can never resolve to the same grant.
 *
 * Returns null while locked or when the source has no stored tree URI.
 */
internal suspend fun resolveTreeUri(
    manager: AppManager,
    source: DirectorySourceSummary,
): String? = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) {
    try {
        manager.getDirectoryTreeUri(source.id)
    } catch (e: Exception) {
        Log.e("DirectorySyncWorker", "getDirectoryTreeUri(${source.id}) failed: ${e.message}")
        null
    }
}
