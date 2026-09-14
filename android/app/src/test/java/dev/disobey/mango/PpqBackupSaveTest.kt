package dev.disobey.mango

import dev.disobey.mango.rust.AppAction
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.ByteArrayOutputStream
import java.io.OutputStream

/**
 * Focused tests for the PPQ backup save flow (finding 9): the create-document
 * result must be consumed once, cancellation/null-stream/write-failure must never
 * mark the backup saved, failure must be observable, and ConfirmPpqBackupSaved may
 * only be dispatched after a real successful write + flush + close.
 */
class PpqBackupSaveTest {

    // ── Result decision table ─────────────────────────────────────────────────

    @Test
    fun `stale result without pending bytes is ignored`() {
        assertEquals(PpqBackupSaveDecision.IGNORE, decidePpqBackupSave(hasPendingBytes = false, uriChosen = false))
        assertEquals(PpqBackupSaveDecision.IGNORE, decidePpqBackupSave(hasPendingBytes = false, uriChosen = true))
    }

    @Test
    fun `picker cancellation is a terminal outcome`() {
        assertEquals(PpqBackupSaveDecision.CANCELLED, decidePpqBackupSave(hasPendingBytes = true, uriChosen = false))
    }

    @Test
    fun `chosen location proceeds to a real write`() {
        assertEquals(PpqBackupSaveDecision.WRITE, decidePpqBackupSave(hasPendingBytes = true, uriChosen = true))
    }

    // ── Write execution incl. failure modes ───────────────────────────────────

    /** Stream that can be made to fail at write, flush, or close. */
    private class ProbeStream(
        private val sink: ByteArrayOutputStream = ByteArrayOutputStream(),
        private val failOnWrite: Boolean = false,
        private val failOnFlush: Boolean = false,
        private val failOnClose: Boolean = false,
    ) : OutputStream() {
        var closed = false
        override fun write(b: Int) {
            if (failOnWrite) throw java.io.IOException("write failed")
            sink.write(b)
        }

        override fun write(b: ByteArray, off: Int, len: Int) {
            if (failOnWrite) throw java.io.IOException("write failed")
            sink.write(b, off, len)
        }

        override fun flush() {
            if (failOnFlush) throw java.io.IOException("flush failed")
        }

        override fun close() {
            closed = true
            if (failOnClose) throw java.io.IOException("close failed")
        }

        fun writtenBytes() = sink.toByteArray()
    }

    @Test
    fun `successful write flush and close saves the exact bytes`() {
        val stream = ProbeStream()
        val bytes = byteArrayOf(1, 2, 3, 4, 5)
        val result = writePpqBackupBytes(bytes) { stream }
        assertEquals(PpqBackupWriteResult.SAVED, result)
        assertTrue(stream.closed)
        assertTrue(stream.writtenBytes().contentEquals(bytes))
    }

    @Test
    fun `null output stream fails and nothing is marked saved`() {
        // SAF can hand back a null stream for revoked/removed providers.
        val result = writePpqBackupBytes(byteArrayOf(9)) { null }
        assertEquals(PpqBackupWriteResult.FAILED, result)
    }

    @Test
    fun `failed write closes the stream and reports failure`() {
        val stream = ProbeStream(failOnWrite = true)
        val result = writePpqBackupBytes(byteArrayOf(1, 2, 3)) { stream }
        assertEquals(PpqBackupWriteResult.FAILED, result)
        // use{} must still close the partially-written stream.
        assertTrue(stream.closed)
        assertTrue(stream.writtenBytes().isEmpty())
    }

    @Test
    fun `failed flush reports failure`() {
        val result = writePpqBackupBytes(byteArrayOf(1)) { ProbeStream(failOnFlush = true) }
        assertEquals(PpqBackupWriteResult.FAILED, result)
    }

    @Test
    fun `failed close reports failure even after a successful write`() {
        // Only confirm after a fully durable write: a close failure means the save
        // cannot be trusted, so the backup must not be marked saved.
        val stream = ProbeStream(failOnClose = true)
        val result = writePpqBackupBytes(byteArrayOf(1, 2, 3)) { stream }
        assertEquals(PpqBackupWriteResult.FAILED, result)
        assertTrue(stream.closed)
    }

    // ── Outcome application: confirmation only on real success ────────────────

    private fun recordedActions(): Pair<MutableList<AppAction>, MutableList<String>> =
        mutableListOf<AppAction>() to mutableListOf<String>()

    @Test
    fun `saved outcome dispatches ConfirmPpqBackupSaved and nothing else`() {
        val (actions, notices) = recordedActions()
        applyPpqBackupSaveOutcome(PpqBackupSaveOutcome.SAVED, actions::add, notices::add)
        assertEquals(listOf<AppAction>(AppAction.ConfirmPpqBackupSaved), actions)
        assertTrue(notices.isEmpty())
    }

    @Test
    fun `cancelled outcome notifies user and never confirms backup`() {
        val (actions, notices) = recordedActions()
        applyPpqBackupSaveOutcome(PpqBackupSaveOutcome.CANCELLED, actions::add, notices::add)
        assertFalse(actions.contains(AppAction.ConfirmPpqBackupSaved))
        assertEquals(1, notices.size)
        assertTrue(notices[0].contains("not saved", ignoreCase = true))
    }

    @Test
    fun `write failure notifies user and never confirms backup`() {
        val (actions, notices) = recordedActions()
        applyPpqBackupSaveOutcome(PpqBackupSaveOutcome.WRITE_FAILED, actions::add, notices::add)
        assertFalse(actions.contains(AppAction.ConfirmPpqBackupSaved))
        assertEquals(1, notices.size)
        assertTrue(notices[0].contains("could not be saved", ignoreCase = true))
    }
}
