package dev.disobey.mango

import androidx.fragment.app.FragmentActivity
import java.lang.ref.WeakReference

/**
 * Rebindable biometric provider (plan §7.5).
 *
 * The Android AppManager singleton is constructed once and previously captured a
 * single activity via a weak reference — stale after rotation/recreation, so a
 * BiometricPrompt launched from it silently failed. This proxy is created once
 * and injected into Rust; every MainActivity.onCreate() attaches the CURRENT
 * activity, onDestroy() detaches only if this activity still owns the proxy
 * (an old activity can never clear a newer one). Each authenticate() snapshots
 * a live, non-finishing activity; otherwise it fails safe (returns false) —
 * a prompt interrupted by recreation may be retried, never auto-approved.
 */
class RebindableBiometricProvider : dev.disobey.mango.rust.BiometricProvider {
    private val lock = Any()
    private var activityRef: WeakReference<FragmentActivity>? = null

    fun attach(activity: FragmentActivity) {
        synchronized(lock) { activityRef = WeakReference(activity) }
    }

    fun detach(activity: FragmentActivity) {
        synchronized(lock) {
            val current = activityRef?.get()
            if (current === activity) activityRef = null
        }
    }

    private fun currentActivity(): FragmentActivity? = synchronized(lock) {
        activityRef?.get()?.takeIf { !it.isFinishing && !it.isDestroyed }
    }

    override fun biometricStatus(): String {
        return try {
            val act = currentActivity() ?: return "not_available"
            BiometricProviderImpl(act).biometricStatus()
        } catch (_: Exception) {
            "not_available"
        }
    }

    override fun authenticate(reason: String): Boolean {
        val act = currentActivity() ?: return false
        return try {
            // Fresh prompt bound to the CURRENT activity; blocks until the
            // platform callback fires (same contract as BiometricProviderImpl).
            BiometricProviderImpl(act).authenticate(reason)
        } catch (_: Exception) {
            false
        }
    }
}
