package dev.disobey.mango

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
// AGENTS HIDDEN: import android.content.Intent
// AGENTS HIDDEN: import androidx.lifecycle.Lifecycle
// AGENTS HIDDEN: import androidx.lifecycle.LifecycleEventObserver
// AGENTS HIDDEN: import dev.disobey.mango.rust.AppAction
// AGENTS HIDDEN: import dev.disobey.mango.rust.Screen
// AGENTS HIDDEN: import dev.disobey.mango.ui.scheduleAgentWorker
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import dev.disobey.mango.ui.MainApp
import dev.disobey.mango.ui.theme.AppTheme

class MainActivity : ComponentActivity() {
    private lateinit var manager: AppManager

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        manager = AppManager.getInstance(applicationContext)

        // AGENTS HIDDEN: lifecycle observer for scheduleAgentWorker removed until polished
        // lifecycle.addObserver(LifecycleEventObserver { _, event ->
        //     if (event == Lifecycle.Event.ON_STOP) {
        //         manager.state.agentSessions
        //             .filter { it.status == "running" }
        //             .forEach { session -> scheduleAgentWorker(applicationContext, session.id) }
        //     }
        // })

        // AGENTS HIDDEN: handleAgentNotificationIntent(intent)

        val prefs = getSharedPreferences("app_prefs", MODE_PRIVATE)
        var themeMode by mutableStateOf(prefs.getString("theme_mode", "system") ?: "system")

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
                )
            }
        }
    }

    // AGENTS HIDDEN: onNewIntent agent notification routing removed until polished
    // override fun onNewIntent(intent: Intent) {
    //     super.onNewIntent(intent)
    //     handleAgentNotificationIntent(intent)
    // }

    // AGENTS HIDDEN: handleAgentNotificationIntent removed until polished
    // private fun handleAgentNotificationIntent(intent: Intent?) {
    //     val sessionId = intent?.getStringExtra("agent_session_id") ?: return
    //     manager.dispatch(AppAction.LoadAgentSession(sessionId = sessionId))
    //     manager.dispatch(AppAction.PushScreen(screen = Screen.Agents))
    // }
}
