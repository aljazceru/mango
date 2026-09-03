package dev.disobey.mango

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.After
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class PpqBackupCoordinatorTest {
    @Before
    fun resetToDefaults() {
        PpqBackupCoordinator.requestExport = null
        PpqBackupCoordinator.requestImport = null
        PpqBackupCoordinator.onPpqImportResult = null
    }

    @After
    fun tearDown() {
        PpqBackupCoordinator.requestExport = null
        PpqBackupCoordinator.requestImport = null
        PpqBackupCoordinator.onPpqImportResult = null
    }

    @Test
    fun requestExport_callback_is_invoked() {
        var captured: ByteArray? = null
        val payload = byteArrayOf(1, 2, 3)

        PpqBackupCoordinator.requestExport = { bytes -> captured = bytes }
        PpqBackupCoordinator.requestExport?.invoke(payload)

        assertTrue(captured contentEquals payload)
    }

    @Test
    fun requestImport_callback_is_invoked() {
        var invoked = false

        PpqBackupCoordinator.requestImport = { invoked = true }
        PpqBackupCoordinator.requestImport?.invoke()

        assertTrue(invoked)
    }

    @Test
    fun onPpqImportResult_callback_is_invoked() {
        var captured: ByteArray? = null
        val payload = byteArrayOf(4, 5, 6)

        PpqBackupCoordinator.onPpqImportResult = { bytes -> captured = bytes }
        PpqBackupCoordinator.onPpqImportResult?.invoke(payload)

        assertTrue(captured contentEquals payload)
    }

    @Test
    fun null_callbacks_do_not_npe() {
        PpqBackupCoordinator.requestExport = null
        PpqBackupCoordinator.requestImport = null
        PpqBackupCoordinator.onPpqImportResult = null

        assertNull(PpqBackupCoordinator.requestExport?.invoke(byteArrayOf()))
        assertNull(PpqBackupCoordinator.requestImport?.invoke())
        assertNull(PpqBackupCoordinator.onPpqImportResult?.invoke(byteArrayOf()))
    }
}
