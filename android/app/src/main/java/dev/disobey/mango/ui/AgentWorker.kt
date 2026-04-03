package dev.disobey.mango.ui

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.work.*
import dev.disobey.mango.AppManager
import dev.disobey.mango.rust.AppAction
import kotlinx.coroutines.delay

/**
 * WorkManager CoroutineWorker that resumes an agent session in the background.
 *
 * Dispatches ResumeAgentSession via AppManager, polls until the session finishes
 * or 5 minutes elapse, then posts a local completion notification.
 *
 * Per D-15: the notification's PendingIntent carries agent_session_id so the
 * MainActivity can route to the session detail view on tap.
 */
class AgentWorker(
    context: Context,
    params: WorkerParameters
) : CoroutineWorker(context, params) {

    override suspend fun doWork(): Result {
        val sessionId = inputData.getString("session_id") ?: return Result.failure()
        val manager = AppManager.getInstance(applicationContext)

        // Resume the agent session in the Rust actor
        manager.dispatch(AppAction.ResumeAgentSession(sessionId = sessionId))

        // Poll until the session leaves "running" state (max 5 minutes)
        val maxPollMs = 5 * 60 * 1000L
        val pollIntervalMs = 2_000L
        var elapsed = 0L
        while (elapsed < maxPollMs) {
            delay(pollIntervalMs)
            elapsed += pollIntervalMs
            val session = manager.state.agentSessions.find { it.id == sessionId }
            if (session == null || session.status != "running") break
        }

        // Post completion notification with session routing (D-15)
        val session = manager.state.agentSessions.find { it.id == sessionId }
        val sessionTitle = session?.title ?: "Agent Session"
        postCompletionNotification(applicationContext, sessionId, sessionTitle)

        return Result.success()
    }

    private fun postCompletionNotification(
        context: Context,
        sessionId: String,
        sessionTitle: String
    ) {
        val channelId = "agent_completion"
        val notificationManager =
            context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager

        // Create notification channel (required on Android 8+)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                channelId,
                "Agent Tasks",
                NotificationManager.IMPORTANCE_DEFAULT
            ).apply {
                description = "Notifications for completed agent sessions"
            }
            notificationManager.createNotificationChannel(channel)
        }

        // D-15: Intent with agent_session_id so MainActivity can route to agent detail
        val intent = context.packageManager
            .getLaunchIntentForPackage(context.packageName)
            ?.apply {
                putExtra("agent_session_id", sessionId)
                flags = Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP
            }

        val pendingIntent = intent?.let {
            PendingIntent.getActivity(
                context,
                sessionId.hashCode(),
                it,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
            )
        }

        val notification = NotificationCompat.Builder(context, channelId)
            .setSmallIcon(android.R.drawable.ic_dialog_info)
            .setContentTitle("Agent Task Complete")
            .setContentText("$sessionTitle has finished processing.")
            .setPriority(NotificationCompat.PRIORITY_DEFAULT)
            .setContentIntent(pendingIntent)
            .setAutoCancel(true)
            .build()

        notificationManager.notify(sessionId.hashCode(), notification)
    }
}

/**
 * Schedules a one-time AgentWorker for the given session via WorkManager.
 *
 * Uses ExistingWorkPolicy.KEEP so a second schedule for the same session is a no-op
 * if the worker is already running or enqueued.
 */
fun scheduleAgentWorker(context: Context, sessionId: String) {
    val request = OneTimeWorkRequestBuilder<AgentWorker>()
        .setInputData(workDataOf("session_id" to sessionId))
        .setConstraints(
            Constraints.Builder()
                .setRequiredNetworkType(NetworkType.CONNECTED)
                .build()
        )
        .build()

    WorkManager.getInstance(context).enqueueUniqueWork(
        "agent-$sessionId",
        ExistingWorkPolicy.KEEP,
        request
    )
}
