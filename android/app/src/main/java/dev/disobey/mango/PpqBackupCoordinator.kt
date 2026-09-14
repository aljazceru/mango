package dev.disobey.mango

import java.util.concurrent.atomic.AtomicLong

/**
 * Callback registry for PPQ recovery backup import/export.
 *
 * [MainActivity] binds the activity-scoped SAF launchers here before [setContent]
 * using a fresh owner token per activity instance, and detaches with the same
 * token in `onDestroy`. Ownership binding matters because a STALE activity can be
 * destroyed after its replacement registered (config-change ordering): a stale
 * detach is a no-op and can never wipe the newer activity's hooks.
 */
object PpqBackupCoordinator {
    private val ownerCounter = AtomicLong(0)

    /** Fresh identity for one activity instance's SAF-hook ownership. */
    fun nextOwnerToken(): Long = ownerCounter.incrementAndGet()

    /** Token of the activity that currently owns the SAF hooks; null when detached. */
    private var ownerToken: Long? = null

    /**
     * Request that the owning Activity write the given encrypted backup bytes to a
     * user-chosen document via ACTION_CREATE_DOCUMENT. Set only via
     * [bindActivityHooks]; nulled by the owner's [detachActivityHooks].
     */
    var requestExport: ((ByteArray) -> Unit)? = null
        private set

    /**
     * Request that the owning Activity start an OpenDocument flow. Set only via
     * [bindActivityHooks]; nulled by the owner's [detachActivityHooks].
     */
    var requestImport: (() -> Unit)? = null
        private set

    /**
     * Called by [MainActivity] when the OpenDocument result delivers bytes.
     * The restore dialog sets this to a closure that captures the password and step-up auth.
     */
    var onPpqImportResult: ((ByteArray) -> Unit)? = null

    /**
     * Encrypted backup bytes awaiting the SAF create-document result.
     *
     * Held here rather than in an Activity field so the save survives activity
     * recreation (rotation) while the system picker is showing. Consumed exactly
     * once on every terminal outcome — success, write failure, or picker
     * cancellation — and dropped when the owning activity finishes.
     *
     * NOTE: no Android-side account identity is attached. Which account a pending
     * export belongs to is validated by the Rust actor when the
     * `ConfirmPpqBackupSaved` notification arrives after a successful file close
     * (ppq_backup_export_credit scope); UI-visible fields are not account identity.
     */
    var pendingExportBytes: ByteArray? = null
        private set

    /**
     * Bind the SAF hooks to the activity identified by [owner], replacing any
     * previous owner's hooks (the previous owner's later detach becomes a no-op).
     */
    fun bindActivityHooks(owner: Long, requestExport: (ByteArray) -> Unit, requestImport: () -> Unit) {
        ownerToken = owner
        this.requestExport = requestExport
        this.requestImport = requestImport
    }

    /** Stage [bytes] for the pending create-document flow. */
    fun stagePendingExport(bytes: ByteArray) {
        pendingExportBytes = bytes
    }

    /** Take (and clear) the staged bytes; null when nothing is pending. */
    fun consumePendingExport(): ByteArray? {
        val pending = pendingExportBytes
        pendingExportBytes = null
        return pending
    }

    /**
     * Detach the SAF hooks, but ONLY if [owner] still owns them — a stale activity
     * destroyed after its replacement bound must not wipe the newer owner's hooks.
     *
     * [clearPendingExport] must be true only when the owning activity is finishing:
     * during a configuration change the staged bytes must survive for the recreated
     * activity to complete the save. [onPpqImportResult] is dropped with the hooks —
     * it captures the old composition's backup password and PIN.
     */
    fun detachActivityHooks(owner: Long, clearPendingExport: Boolean) {
        if (ownerToken != owner) return
        ownerToken = null
        requestExport = null
        requestImport = null
        onPpqImportResult = null
        if (clearPendingExport) {
            pendingExportBytes = null
        }
    }
}
