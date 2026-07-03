package dev.disobey.mango

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

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
}
