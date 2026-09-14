package dev.disobey.mango

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.After
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Instrumented coverage for the token-bound PPQ SAF hook ownership (runs on a
 * device/emulator; compiled by CI even when no device is attached).
 */
@RunWith(AndroidJUnit4::class)
class PpqBackupCoordinatorTest {
    private var owner = 0L
    private var stale = 0L

    @Before
    fun setUp() {
        owner = PpqBackupCoordinator.nextOwnerToken()
        stale = PpqBackupCoordinator.nextOwnerToken()
    }

    @After
    fun tearDown() {
        PpqBackupCoordinator.detachActivityHooks(owner, clearPendingExport = true)
        PpqBackupCoordinator.detachActivityHooks(stale, clearPendingExport = true)
        PpqBackupCoordinator.onPpqImportResult = null
    }

    @Test
    fun requestExport_callback_is_invoked_with_staged_bytes() {
        var captured: ByteArray? = null
        PpqBackupCoordinator.bindActivityHooks(owner, { bytes -> captured = bytes }) {}

        PpqBackupCoordinator.requestExport?.invoke(byteArrayOf(1, 2, 3))

        assertTrue(captured contentEquals byteArrayOf(1, 2, 3))
    }

    @Test
    fun requestImport_callback_is_invoked() {
        var invoked = false
        PpqBackupCoordinator.bindActivityHooks(owner, {}) { invoked = true }

        PpqBackupCoordinator.requestImport?.invoke()

        assertTrue(invoked)
    }

    @Test
    fun onPpqImportResult_callback_is_invoked() {
        var captured: ByteArray? = null
        PpqBackupCoordinator.onPpqImportResult = { bytes -> captured = bytes }

        PpqBackupCoordinator.onPpqImportResult?.invoke(byteArrayOf(4, 5, 6))

        assertTrue(captured contentEquals byteArrayOf(4, 5, 6))
    }

    @Test
    fun stale_owner_detach_does_not_remove_current_owner_hooks() {
        val export: (ByteArray) -> Unit = {}
        PpqBackupCoordinator.bindActivityHooks(owner, export) {}

        // A destroyed stale activity detaches after the newer one bound.
        PpqBackupCoordinator.detachActivityHooks(stale, clearPendingExport = true)

        assertSame(export, PpqBackupCoordinator.requestExport)
        assertTrue(PpqBackupCoordinator.requestImport != null)

        // The real owner can still detach.
        PpqBackupCoordinator.detachActivityHooks(owner, clearPendingExport = true)
        assertNull(PpqBackupCoordinator.requestExport)
    }

    @Test
    fun detached_hooks_are_not_invokable() {
        PpqBackupCoordinator.bindActivityHooks(owner, {}) {}
        PpqBackupCoordinator.detachActivityHooks(owner, clearPendingExport = false)

        var invoked = false
        val export = PpqBackupCoordinator.requestExport
        export?.invoke(byteArrayOf())
        assertFalse(invoked)
        assertNull(PpqBackupCoordinator.requestImport)
    }
}
