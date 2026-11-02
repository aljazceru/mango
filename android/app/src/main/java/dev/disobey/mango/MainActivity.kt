package dev.disobey.mango

import android.os.Bundle
import android.content.Intent
import android.view.WindowManager
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import android.net.Uri
import androidx.lifecycle.lifecycleScope
import dev.disobey.mango.rust.AppAction
import dev.disobey.mango.rust.Screen
import dev.disobey.mango.ui.DirectorySyncWorker
import dev.disobey.mango.ui.resolveTreeUri
import dev.disobey.mango.ui.scheduleAgentWorker
import dev.disobey.mango.ui.syncDirectory
import kotlinx.coroutines.launch
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.disobey.mango.ui.MainApp
import dev.disobey.mango.ui.ppq.ppqBackupFileName
import dev.disobey.mango.ui.theme.AppTheme
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.ByteArrayOutputStream
import java.io.OutputStream



/** How to react to a create-document result for a pending PPQ backup. Pure (unit-tested). */
internal enum class PpqBackupSaveDecision {
    /** Result arrived with no pending export — stale/duplicate result; do nothing. */
    IGNORE,
    /** User dismissed the picker without choosing a location; nothing was written. */
    CANCELLED,
    /** A location was chosen — attempt the write; saved/failed is decided by the write. */
    WRITE,
}

/**
 * Decide the reaction to a create-document result. Cancel never confirms the backup,
 * and every terminal outcome clears the pending export bytes (done by the caller).
 */
internal fun decidePpqBackupSave(hasPendingBytes: Boolean, uriChosen: Boolean): PpqBackupSaveDecision = when {
    !hasPendingBytes -> PpqBackupSaveDecision.IGNORE
    !uriChosen -> PpqBackupSaveDecision.CANCELLED
    else -> PpqBackupSaveDecision.WRITE
}

/** Result of attempting the backup write itself. Pure (unit-tested). */
internal enum class PpqBackupWriteResult { SAVED, FAILED }

/**
 * Write the encrypted PPQ backup bytes through [openStream]. Returns [PpqBackupWriteResult.SAVED]
 * only after a successful open, write, flush, AND close — a null stream, a failed write,
 * or a failed close all yield FAILED so the backup is never marked saved without a real
 * durable write. Pure JVM (no Android types) so failure modes are unit-testable.
 */
internal fun writePpqBackupBytes(bytes: ByteArray, openStream: () -> OutputStream?): PpqBackupWriteResult =
    try {
        val stream = openStream()
        if (stream == null) {
            PpqBackupWriteResult.FAILED
        } else {
            stream.use { out ->
                out.write(bytes)
                out.flush()
            }
            PpqBackupWriteResult.SAVED
        }
    } catch (e: Exception) {
        android.util.Log.e("MainActivity", "PPQ backup write failed: ${e.message}")
        PpqBackupWriteResult.FAILED
    }

/** Terminal, user/app-visible outcome of a PPQ backup save flow. */
internal enum class PpqBackupSaveOutcome { SAVED, CANCELLED, WRITE_FAILED }

/**
 * Apply a terminal backup-save outcome: notify Rust (ConfirmPpqBackupSaved) ONLY
 * on a real successful write+close — the actor validates that the confirmation
 * still matches the account whose export is pending (ppq_backup_export_credit
 * scope), so a stale completion can never mark a different/restored/wiped account
 * as backed up. Cancellation and failure are made observable to the user instead
 * of failing silently. Pure (unit-tested).
 */
internal fun applyPpqBackupSaveOutcome(
    outcome: PpqBackupSaveOutcome,
    dispatch: (AppAction) -> Unit,
    notifyUser: (String) -> Unit,
) {
    when (outcome) {
        PpqBackupSaveOutcome.SAVED -> dispatch(AppAction.ConfirmPpqBackupSaved)
        PpqBackupSaveOutcome.CANCELLED -> notifyUser("Backup cancelled — your PPQ backup was not saved")
        PpqBackupSaveOutcome.WRITE_FAILED -> notifyUser("PPQ backup could not be saved — please try again")
    }
}

class MainActivity : AppCompatActivity() {
    /** Null until the heavy init (Keystore, ONNX model, native probe) finishes
     * off the main thread; a startup failure is surfaced via [startupError]. */
    private var manager by mutableStateOf<AppManager?>(null)

    /** Non-null when AppManager construction threw — a retryable recovery state. */
    private var startupError by mutableStateOf<Throwable?>(null)

    /** Identity of THIS activity instance for [PpqBackupCoordinator] hook ownership,
     * so a stale activity's onDestroy can never detach a newer activity's hooks. */
    private val safOwnerToken: Long = PpqBackupCoordinator.nextOwnerToken()

    /** Last resume-based PPQ invoice refresh; debounce at ~2s. */
    private var lastPpqResumeCheckAt: Long = 0

    private val createDocumentLauncher = registerForActivityResult(
        ActivityResultContracts.CreateDocument("application/octet-stream"),
    ) { uri ->
        handlePpqBackupSaveResult(uri)
    }

    private val openDocumentLauncher = registerForActivityResult(
        ActivityResultContracts.OpenDocument(),
    ) { uri ->
        if (uri == null) {
            // Threat review (medium): picker cancelled — drop the closure that
            // captures the backup password + PIN.
            PpqBackupCoordinator.onPpqImportResult = null
            return@registerForActivityResult
        }
        lifecycleScope.launch(Dispatchers.IO) {
            try {
                // Threat review (medium): cap the read at the Rust-side
                // recovery-file limit (+1 to detect oversize) before copying.
                val maxBytes = 64 * 1024L + 1
                val bytes = contentResolver.openInputStream(uri)?.use { inStream ->
                    ByteArrayOutputStream().use { out ->
                        val buf = ByteArray(8192)
                        while (out.size() <= maxBytes) {
                            val n = inStream.read(buf)
                            if (n <= 0) break
                            out.write(buf, 0, n)
                        }
                        if (out.size() > maxBytes) null else out.toByteArray()
                    }
                }
                withContext(Dispatchers.Main) {
                    val handler = PpqBackupCoordinator.onPpqImportResult
                    PpqBackupCoordinator.onPpqImportResult = null
                    if (bytes != null && handler != null) {
                        handler(bytes)
                    } else if (bytes != null) {
                        // The restore dialog died with a recreated composition — its
                        // password capture is gone. Fail observably instead of silently
                        // dropping the user's picked file.
                        notifyPpqBackupUser("Restore cancelled — please try again")
                    }
                }
            } catch (e: Exception) {
                PpqBackupCoordinator.onPpqImportResult = null
                android.util.Log.e("MainActivity", "PPQ backup read failed: ${e.message}")
            }
        }
    }

    /**
     * Finding 9 fix: a create-document result is consumed exactly once on every
     * terminal outcome. The staged bytes are cleared immediately (they also survive
     * activity recreation in [PpqBackupCoordinator]), Rust is notified of the save
     * ONLY after a real successful write+flush+close (the actor-side pending-export
     * scope decides whether the confirmation still applies to the current account),
     * and cancellation, a null output stream, or a failed write are surfaced to the
     * user instead of passing silently.
     */
    private fun handlePpqBackupSaveResult(uri: Uri?) {
        val bytes = PpqBackupCoordinator.consumePendingExport()
        when (decidePpqBackupSave(hasPendingBytes = bytes != null, uriChosen = uri != null)) {
            PpqBackupSaveDecision.IGNORE -> {}
            PpqBackupSaveDecision.CANCELLED ->
                applyPpqBackupSaveOutcome(PpqBackupSaveOutcome.CANCELLED, { a -> manager?.dispatch(a) }, ::notifyPpqBackupUser)
            PpqBackupSaveDecision.WRITE -> lifecycleScope.launch(Dispatchers.IO) {
                val write = writePpqBackupBytes(bytes!!) { contentResolver.openOutputStream(uri!!) }
                val outcome = if (write == PpqBackupWriteResult.SAVED) {
                    PpqBackupSaveOutcome.SAVED
                } else {
                    PpqBackupSaveOutcome.WRITE_FAILED
                }
                withContext(Dispatchers.Main) {
                    applyPpqBackupSaveOutcome(outcome, { a -> manager?.dispatch(a) }, ::notifyPpqBackupUser)
                }
            }
        }
    }

    /** Surface a backup-save notice via app state so it is visible in the UI. */
    private fun notifyPpqBackupUser(message: String) {
        manager?.dispatch(AppAction.ShowToast(message = message))
    }

    override fun onPause() {
        super.onPause()
        // The timestamp lives in the process-wide AppManager (monotonic clock):
        // activity recreation and back-to-launcher + relaunch can no longer
        // reset it to zero and skip the configured lock timeout.
        manager?.onAppBackgrounded()
    }

    override fun onDestroy() {
        // Detach ONLY this instance's hooks (a stale activity destroyed after its
        // replacement bound must not wipe the newer owner's hooks). The SAF launchers
        // die with this activity. Pending export bytes survive a config change (the
        // recreated activity completes the save) and are dropped only when finishing.
        PpqBackupCoordinator.detachActivityHooks(safOwnerToken, clearPendingExport = isFinishing)
        manager?.biometricRebindable?.detach(this)
        super.onDestroy()
    }

    override fun onResume() {
        super.onResume()
        val mgr = manager ?: return
        // Enforce the process-wide deadline on EVERY resume: a recreated or
        // re-launched activity gets the same answer instead of timestamp zero.
        if (mgr.enforceLockDeadline()) {
            mgr.dispatch(AppAction.LockApp)
            return
        }

        // Phase PPQ: resume-based check for pending Lightning invoices.
        if (mgr.state.router.currentScreen !is Screen.Locked) {
            val ppq = mgr.state.ppq
            if (ppq.funding != null) {
                val now = System.currentTimeMillis()
                if (now - lastPpqResumeCheckAt > 2_000) {
                    mgr.dispatch(AppAction.CheckPpqTopup)
                    mgr.dispatch(AppAction.RefreshPpqAccount)
                    lastPpqResumeCheckAt = now
                }
            }
        }

        // Phase 32 Plan 06: foreground-resume sync for all directory sources
        // (D-22 belt-and-braces alongside the 15-minute WorkManager schedule).
        // Skipped when the app is locked (matches iOS ScenePhase gating in plan 32-05).
        if (mgr.state.router.currentScreen !is Screen.Locked) {
            val ctx = applicationContext
            val sources = mgr.state.directorySources
            if (sources.isNotEmpty()) {
                lifecycleScope.launch {
                    for (source in sources) {
                        val tree = resolveTreeUri(mgr, source) ?: continue
                        try {
                            syncDirectory(ctx, source, Uri.parse(tree)) { action ->
                                mgr.dispatch(action)
                            }
                        } catch (t: Throwable) {
                            android.util.Log.e(
                                "MainActivity",
                                "onResume dir sync failed for ${source.id}: ${t.message}",
                            )
                        }
                    }
                }
            }
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        // FLAG_SECURE before any content: covers chat, PIN and recovery
        // screens against screenshots, recents thumbnails and recording.
        window.setFlags(
            WindowManager.LayoutParams.FLAG_SECURE,
            WindowManager.LayoutParams.FLAG_SECURE,
        )
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)

        // PPQ backup SAF launchers: registered before setContent under this
        // activity's owner token; composables request via the coordinator.
        PpqBackupCoordinator.bindActivityHooks(
            owner = safOwnerToken,
            requestExport = ::launchPpqBackupExport,
            requestImport = ::launchPpqBackupImport,
        )

        // Phase 32 Plan 06: enqueue the 15-minute periodic directory-sync worker
        // (D-23). KEEP policy means this is idempotent across config changes.
        DirectorySyncWorker.enqueue(applicationContext)

        lifecycle.addObserver(LifecycleEventObserver { _, event ->
            if (FeatureFlags.AGENTS_ENABLED && event == Lifecycle.Event.ON_STOP) {
                manager?.state?.agentSessions
                    ?.filter { it.status == "running" }
                    ?.forEach { session -> scheduleAgentWorker(applicationContext, session.id) }
            }
        })

        handleAgentNotificationIntent(intent)

        // Heavy startup (Keystore/EncryptedSharedPreferences, ONNX model copy,
        // native capability probe, Rust core) runs off the main thread; the UI
        // shows a blank frame (or the recovery screen) until it completes.
        startManagerInit()

        val prefs = getSharedPreferences("app_prefs", MODE_PRIVATE)
        var themeMode by mutableStateOf(prefs.getString("theme_mode", "system") ?: "system")
        var fontSize by mutableStateOf(prefs.getString("font_size", "normal") ?: "normal")

        setContent {
            val useDarkTheme = when (themeMode) {
                "dark" -> true
                "light" -> false
                else -> isSystemInDarkTheme()
            }
            AppTheme(darkTheme = useDarkTheme) {
                val mgr = manager
                val error = startupError
                when {
                    error != null -> StartupRecovery(
                        onRetry = { retryStartup() },
                    )
                    mgr != null -> MainApp(
                        manager = mgr,
                        themeMode = themeMode,
                        onThemeModeChanged = { newMode ->
                            themeMode = newMode
                            prefs.edit().putString("theme_mode", newMode).apply()
                        },
                        fontSize = fontSize,
                        onFontSizeChanged = { newSize ->
                            fontSize = newSize
                            prefs.edit().putString("font_size", newSize).apply()
                        },
                    )
                    else -> Box(modifier = Modifier.fillMaxSize()) // init in progress
                }
            }
        }
    }

    /** Build/reuse the singleton off the main thread; on success attach the
     * biometric bridge to THIS activity and enforce any armed lock deadline. */
    private fun startManagerInit() {
        lifecycleScope.launch {
            try {
                val mgr = AppManager.warmInstance(applicationContext, this@MainActivity)
                startupError = null
                manager = mgr
                // §7.5: rebind the biometric bridge to THIS activity every recreation.
                mgr.biometricRebindable?.attach(this@MainActivity)
                // A deadline armed before this activity existed (back+relaunch)
                // still fires here.
                if (mgr.enforceLockDeadline()) {
                    mgr.dispatch(AppAction.LockApp)
                }
            } catch (e: Throwable) {
                android.util.Log.e("MainActivity", "AppManager init failed", e)
                startupError = e
            }
        }
    }

    private fun retryStartup() {
        startupError = null
        startManagerInit()
    }

    /** Start the create-document flow for the staged encrypted bytes. */
    private fun launchPpqBackupExport(bytes: ByteArray) {
        PpqBackupCoordinator.stagePendingExport(bytes)
        createDocumentLauncher.launch(ppqBackupFileName())
    }

    /** Start the open-document flow for restore. */
    private fun launchPpqBackupImport() {
        openDocumentLauncher.launch(arrayOf("application/octet-stream", "*/*"))
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handleAgentNotificationIntent(intent)
    }

    private fun handleAgentNotificationIntent(intent: Intent?) {
        if (!FeatureFlags.AGENTS_ENABLED) return
        val sessionId = intent?.getStringExtra("agent_session_id") ?: return
        manager?.dispatch(AppAction.LoadAgentSession(sessionId = sessionId))
        manager?.dispatch(AppAction.PushScreen(screen = Screen.Agents))
    }
}

/**
 * Shown when Keystore/EncryptedSharedPreferences/Rust-core construction failed
 * before any app UI existed. Retry is the only action — the failure path never
 * touches user files or credentials, so nothing is offered to clear.
 */
@androidx.compose.runtime.Composable
private fun StartupRecovery(onRetry: () -> Unit) {
    Box(
        modifier = Modifier.fillMaxSize().padding(32.dp),
        contentAlignment = Alignment.Center,
    ) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Text(
                "Mango could not open its secure storage.",
                style = MaterialTheme.typography.titleMedium,
            )
            Text(
                "Your data was not modified. Retry, or restart the app if this persists.",
                style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.padding(top = 8.dp, bottom = 16.dp),
            )
            Button(onClick = onRetry) { Text("Retry") }
        }
    }
}
