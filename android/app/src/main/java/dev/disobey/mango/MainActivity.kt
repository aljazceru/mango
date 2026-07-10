package dev.disobey.mango

import android.os.Bundle
import android.content.Intent
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
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
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import dev.disobey.mango.ui.MainApp
import dev.disobey.mango.ui.theme.AppTheme

internal fun shouldLockAfterBackground(backgroundedAt: Long, now: Long, timeoutSeconds: Long): Boolean {
    if (backgroundedAt <= 0 || timeoutSeconds < 0) {
        return false
    }
    val elapsed = now - backgroundedAt
    return elapsed >= timeoutSeconds * 1000L
}

class MainActivity : AppCompatActivity() {
    private lateinit var manager: AppManager

    /** Timestamp (millis) when the app last moved to background (D-10). 0 = not backgrounded. */
    private var backgroundedAt: Long = 0

    override fun onPause() {
        super.onPause()
        backgroundedAt = System.currentTimeMillis()
    }

    override fun onResume() {
        super.onResume()
        if (backgroundedAt > 0) {
            val now = System.currentTimeMillis()
            val timeoutSeconds = manager.state.lockTimeoutSeconds
            // -1 = Never. 0 = Immediately (always lock). Any positive value: lock if exceeded.
            if (shouldLockAfterBackground(backgroundedAt, now, timeoutSeconds)) {
                manager.dispatch(AppAction.LockApp)
                backgroundedAt = 0
                return
            }
            backgroundedAt = 0
        }

        // Phase 32 Plan 06: foreground-resume sync for all directory sources
        // (D-22 belt-and-braces alongside the 15-minute WorkManager schedule).
        // Skipped when the app is locked (matches iOS ScenePhase gating in plan 32-05).
        if (manager.state.router.currentScreen !is Screen.Locked) {
            val ctx = applicationContext
            val sources = manager.state.directorySources
            if (sources.isNotEmpty()) {
                lifecycleScope.launch {
                    for (source in sources) {
                        val tree = resolveTreeUri(ctx, source) ?: continue
                        try {
                            syncDirectory(ctx, source, Uri.parse(tree)) { action ->
                                manager.dispatch(action)
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
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        manager = AppManager.getInstance(applicationContext, this)

        // Phase 32 Plan 06: enqueue the 15-minute periodic directory-sync worker
        // (D-23). KEEP policy means this is idempotent across config changes.
        DirectorySyncWorker.enqueue(applicationContext)

        lifecycle.addObserver(LifecycleEventObserver { _, event ->
            if (FeatureFlags.AGENTS_ENABLED && event == Lifecycle.Event.ON_STOP) {
                manager.state.agentSessions
                    .filter { it.status == "running" }
                    .forEach { session -> scheduleAgentWorker(applicationContext, session.id) }
            }
        })

        handleAgentNotificationIntent(intent)

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
                MainApp(
                    manager = manager,
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
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handleAgentNotificationIntent(intent)
    }

    private fun handleAgentNotificationIntent(intent: Intent?) {
        if (!FeatureFlags.AGENTS_ENABLED) return
        val sessionId = intent?.getStringExtra("agent_session_id") ?: return
        manager.dispatch(AppAction.LoadAgentSession(sessionId = sessionId))
        manager.dispatch(AppAction.PushScreen(screen = Screen.Agents))
    }
}
