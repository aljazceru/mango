package dev.disobey.mango

import androidx.biometric.BiometricManager
import androidx.biometric.BiometricPrompt
import androidx.core.content.ContextCompat
import androidx.fragment.app.FragmentActivity
import dev.disobey.mango.rust.BiometricProvider
import java.lang.ref.WeakReference
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.CountDownLatch

/**
 * Android implementation of the Rust BiometricProvider callback interface.
 *
 * Uses BiometricPrompt (Class 3 / BIOMETRIC_STRONG) gated behind a CountDownLatch bridge
 * so that Rust's blocking `authenticate()` call blocks the actor thread until the Android
 * async callback fires (D-22, Pitfall 2 from 28-CONTEXT.md).
 *
 * The activity is held via WeakReference to avoid leaking it across orientation changes or
 * if the activity is destroyed while the biometric prompt is showing (T-28-21).
 */
class BiometricProviderImpl(activity: FragmentActivity) : BiometricProvider {

    private val activityRef = WeakReference(activity)

    /**
     * Check whether BIOMETRIC_STRONG is available and enrolled on this device (D-22).
     * Returns: "available", "not_enrolled", or "not_available".
     */
    override fun biometricStatus(): String {
        val activity = activityRef.get() ?: return "not_available"
        val manager = BiometricManager.from(activity)
        return when (manager.canAuthenticate(BiometricManager.Authenticators.BIOMETRIC_STRONG)) {
            BiometricManager.BIOMETRIC_SUCCESS -> "available"
            BiometricManager.BIOMETRIC_ERROR_NONE_ENROLLED -> "not_enrolled"
            BiometricManager.BIOMETRIC_ERROR_NO_HARDWARE,
            BiometricManager.BIOMETRIC_ERROR_HW_UNAVAILABLE,
            BiometricManager.BIOMETRIC_ERROR_UNSUPPORTED,
            BiometricManager.BIOMETRIC_ERROR_SECURITY_UPDATE_REQUIRED -> "not_supported"
            else -> "not_available"
        }
    }

    /**
     * Attempt biometric authentication with the given localized reason string.
     *
     * This method BLOCKS the calling thread (Rust actor thread) using a CountDownLatch until
     * the BiometricPrompt callback fires. BiometricPrompt must be shown on the UI thread,
     * so we use activity.runOnUiThread { } to create and show it there.
     *
     * Returns true on success, false on failure or cancellation.
     * Returns false immediately if the activity WeakReference was GC'd (T-28-21).
     */
    override fun authenticate(reason: String): Boolean {
        val activity = activityRef.get() ?: return false

        val latch = CountDownLatch(1)
        val succeeded = AtomicBoolean(false)

        activity.runOnUiThread {
            val executor = ContextCompat.getMainExecutor(activity)

            val promptInfo = BiometricPrompt.PromptInfo.Builder()
                .setTitle("Unlock")
                .setDescription(reason)
                .setNegativeButtonText("Use PIN")
                .setAllowedAuthenticators(BiometricManager.Authenticators.BIOMETRIC_STRONG)
                .build()

            val biometricPrompt = BiometricPrompt(
                activity,
                executor,
                object : BiometricPrompt.AuthenticationCallback() {
                    override fun onAuthenticationSucceeded(
                        result: BiometricPrompt.AuthenticationResult
                    ) {
                        succeeded.set(true)
                        latch.countDown()
                    }

                    override fun onAuthenticationFailed() {
                        // Biometric not recognized — user can retry, don't count down yet.
                        // The latch will only count down on error or cancel.
                    }

                    override fun onAuthenticationError(errorCode: Int, errString: CharSequence) {
                        // Terminal error (user cancelled, too many attempts, etc.)
                        latch.countDown()
                    }
                }
            )

            biometricPrompt.authenticate(promptInfo)
        }

        // Block the Rust actor thread until the callback fires.
        latch.await()
        return succeeded.get()
    }
}
