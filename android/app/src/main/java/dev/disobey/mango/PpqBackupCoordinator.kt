package dev.disobey.mango

/**
 * Callback registry for PPQ recovery backup import/export.
 *
 * [MainActivity] registers the activity-scoped SAF launchers here before [setContent].
 * Composables request import/export via these hooks so the launchers stay in the Activity
 * lifecycle and can be registered before setContent.
 */
object PpqBackupCoordinator {
    /**
     * Request that the Activity write the given encrypted backup bytes to a user-chosen
     * document via ACTION_CREATE_DOCUMENT.
     */
    var requestExport: ((ByteArray) -> Unit)? = null

    /**
     * Request that the Activity start an OpenDocument flow. The caller must set
     * [onPpqImportResult] before invoking this.
     */
    var requestImport: (() -> Unit)? = null

    /**
     * Called by [MainActivity] when the OpenDocument result delivers bytes.
     * The restore dialog sets this to a closure that captures the password and step-up auth.
     */
    var onPpqImportResult: ((ByteArray) -> Unit)? = null
}
