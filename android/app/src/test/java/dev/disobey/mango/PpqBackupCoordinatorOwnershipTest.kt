package dev.disobey.mango

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/**
 * Focused tests for PpqBackupCoordinator ownership binding (coordinator follow-up
 * issue 2): owner tokens so a stale activity's destruction can never wipe a newer
 * activity's SAF hooks, staged export bytes surviving rotation, and one-shot
 * consumption. Account-identity validation for the eventual confirmation is done
 * by the Rust actor (ppq_backup_export_credit) — deliberately not re-modeled here.
 */
class PpqBackupCoordinatorOwnershipTest {
    private var ownerA = 0L
    private var ownerB = 0L

    @Before
    fun setUp() {
        ownerA = PpqBackupCoordinator.nextOwnerToken()
        ownerB = PpqBackupCoordinator.nextOwnerToken()
    }

    @After
    fun tearDown() {
        // Detach whichever owner currently holds the hooks so suites don't leak.
        PpqBackupCoordinator.detachActivityHooks(ownerA, clearPendingExport = true)
        PpqBackupCoordinator.detachActivityHooks(ownerB, clearPendingExport = true)
        PpqBackupCoordinator.onPpqImportResult = null
    }

    // ── Stale-detach protection ───────────────────────────────────────────────

    @Test
    fun `bind registers hooks and detaching the owner clears them`() {
        val export: (ByteArray) -> Unit = {}
        val import: () -> Unit = {}
        PpqBackupCoordinator.bindActivityHooks(ownerA, export, import)
        assertSame(export, PpqBackupCoordinator.requestExport)
        assertSame(import, PpqBackupCoordinator.requestImport)

        PpqBackupCoordinator.detachActivityHooks(ownerA, clearPendingExport = false)
        assertNull(PpqBackupCoordinator.requestExport)
        assertNull(PpqBackupCoordinator.requestImport)
    }

    @Test
    fun `stale activity detach cannot wipe a newer activity's hooks`() {
        // Config-change ordering: old activity binds, NEW activity binds, then the
        // OLD activity's onDestroy runs last — it must be a no-op.
        val exportOld: (ByteArray) -> Unit = {}
        val exportNew: (ByteArray) -> Unit = {}
        PpqBackupCoordinator.bindActivityHooks(ownerA, exportOld) {}
        PpqBackupCoordinator.bindActivityHooks(ownerB, exportNew) {}

        PpqBackupCoordinator.detachActivityHooks(ownerA, clearPendingExport = true)

        assertSame("newer owner keeps its hooks", exportNew, PpqBackupCoordinator.requestExport)
        assertTrue(PpqBackupCoordinator.requestImport != null)
    }

    @Test
    fun `stale detach keeps staged export and import closure`() {
        val bytes = byteArrayOf(7)
        PpqBackupCoordinator.bindActivityHooks(ownerB, {}) {}
        PpqBackupCoordinator.stagePendingExport(bytes)
        PpqBackupCoordinator.onPpqImportResult = { }

        // An unrelated (stale) owner detaching must not touch the new owner's state.
        PpqBackupCoordinator.detachActivityHooks(ownerA, clearPendingExport = true)

        val pending = PpqBackupCoordinator.consumePendingExport()
        assertTrue(pending != null && pending.contentEquals(bytes))
        assertTrue(PpqBackupCoordinator.onPpqImportResult != null)
    }

    @Test
    fun `owning detach with finish clears staged export and import closure`() {
        PpqBackupCoordinator.bindActivityHooks(ownerA, {}) {}
        PpqBackupCoordinator.stagePendingExport(byteArrayOf(1))
        PpqBackupCoordinator.onPpqImportResult = { }

        PpqBackupCoordinator.detachActivityHooks(ownerA, clearPendingExport = true)

        assertNull(PpqBackupCoordinator.pendingExportBytes)
        assertNull(PpqBackupCoordinator.onPpqImportResult)
    }

    @Test
    fun `owning detach without finish keeps staged export across config change`() {
        PpqBackupCoordinator.bindActivityHooks(ownerA, {}) {}
        PpqBackupCoordinator.stagePendingExport(byteArrayOf(2))

        PpqBackupCoordinator.detachActivityHooks(ownerA, clearPendingExport = false)

        assertTrue(PpqBackupCoordinator.pendingExportBytes != null)
    }

    // ── Staged-export consumption ─────────────────────────────────────────────

    @Test
    fun `consume returns the staged export exactly once`() {
        PpqBackupCoordinator.stagePendingExport(byteArrayOf(3, 1))
        val first = PpqBackupCoordinator.consumePendingExport()
        assertTrue(first != null && first.contentEquals(byteArrayOf(3, 1)))
        assertNull(PpqBackupCoordinator.consumePendingExport())
        assertNull(PpqBackupCoordinator.pendingExportBytes)
    }

    @Test
    fun `restaging replaces any previous pending export`() {
        PpqBackupCoordinator.stagePendingExport(byteArrayOf(1))
        PpqBackupCoordinator.stagePendingExport(byteArrayOf(2))
        val pending = PpqBackupCoordinator.consumePendingExport()
        assertTrue(pending != null && pending.contentEquals(byteArrayOf(2)))
        assertFalse(pending!!.contentEquals(byteArrayOf(1)))
    }
}
