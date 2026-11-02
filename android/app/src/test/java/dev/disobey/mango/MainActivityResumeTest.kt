package dev.disobey.mango

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Tests for the process-wide lock-deadline predicate in AppManager.
 *
 * The background timestamp now lives in the singleton manager on a monotonic
 * clock, so every activity attach/resume enforces the same deadline — the
 * old per-activity `backgroundedAt` reset-to-zero bypass is gone. These tests
 * cover the arithmetic contract MainActivity relies on for Never / Immediately
 * / finite timeouts and clock inversion.
 */
class MainActivityResumeTest {
    @Test
    fun `never timeout does not lock after background`() {
        assertFalse(shouldLockAfterBackground(backgroundedAt = 1_000L, now = 10_000L, timeoutSeconds = -1L))
    }

    @Test
    fun `immediate timeout locks on resume`() {
        assertTrue(shouldLockAfterBackground(backgroundedAt = 1_000L, now = 1_000L, timeoutSeconds = 0L))
    }

    @Test
    fun `finite timeout locks only after elapsed threshold`() {
        assertFalse(shouldLockAfterBackground(backgroundedAt = 1_000L, now = 3_999L, timeoutSeconds = 3L))
        assertTrue(shouldLockAfterBackground(backgroundedAt = 1_000L, now = 4_000L, timeoutSeconds = 3L))
    }

    @Test
    fun `no armed deadline never locks`() {
        // An activity that was never backgrounded (backgroundedAt == 0) must
        // not lock — this is what makes recreation mid-session safe.
        assertFalse(shouldLockAfterBackground(backgroundedAt = 0L, now = 999_999L, timeoutSeconds = 0L))
    }

    @Test
    fun `inverted clock fails closed`() {
        // now < backgroundedAt can only mean the monotonic source was reset;
        // lock rather than silently extend the unlocked window.
        assertTrue(shouldLockAfterBackground(backgroundedAt = 5_000L, now = 4_000L, timeoutSeconds = 300L))
    }
}
