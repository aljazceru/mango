package dev.disobey.mango.ui

import android.content.Intent
import android.os.Build
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Regression tests for the GrapheneOS "Can't use this folder" bug.
 *
 * Root cause documented in
 * .planning/debug/resolved/android-saf-cant-use-folder-grapheneos.md — the picker
 * previously launched with `null` as the initial URI, causing DocumentsUI to open
 * at the internal-storage root where Android 11+ blocks all folder selection.
 *
 * Two regressions are guarded:
 *  1. `useDownloadsUriForSdk` returns false on API >= 29  →  Tests 1-3
 *  2. `PERSISTABLE_URI_FLAGS` drops read or write grant bits  →  Tests 4-6
 *
 * NOTE: Tests use `useDownloadsUriForSdk(sdkInt): Boolean` rather than
 * `initialTreeUriForSdk(sdkInt): Uri?` to stay entirely free of `android.net.Uri`.
 * In the AGP JVM test sandbox, `android.net.Uri` has a package-private constructor
 * and factory methods like `Uri.parse()` throw even with `isReturnDefaultValues = true`
 * (which covers instance methods only, not static fields or constructors). The Boolean
 * predicate is the authoritative SDK-gate that `initialTreeUriForSdk` delegates to, so
 * asserting on it is equivalent to asserting the picker branching logic.
 */
class DirectorySourcePickerTest {

    // -------------------------------------------------------------------------
    // Tests 1-3: Initial URI selection (SDK gate)
    // -------------------------------------------------------------------------

    @Test
    fun `useDownloadsUriForSdk returns true on API 29 (Q)`() {
        // Regression guard: the original bug was equivalent to this returning false
        // and `launcher.launch(null)` being called. If someone reverts the SDK gate,
        // this test fails.
        assertTrue(
            "MUST use Downloads URI on API >= 29 — not doing so re-introduces " +
                "the GrapheneOS 'Can't use this folder' bug",
            useDownloadsUriForSdk(Build.VERSION_CODES.Q),
        )
    }

    @Test
    fun `useDownloadsUriForSdk returns true on API 30 (R)`() {
        // API 30 (Android 11) is where the bug manifested on GrapheneOS / Pixel 9a.
        assertTrue(
            "MUST use Downloads URI on API 30 — this is the exact SDK where the bug appeared",
            useDownloadsUriForSdk(Build.VERSION_CODES.R),
        )
    }

    @Test
    fun `useDownloadsUriForSdk returns false below API 29`() {
        // minSdk = 28 (P). MediaStore.Downloads URI requires API 29+; below that we
        // fall back to null (no initial hint). This is intentional documented behaviour.
        assertFalse(
            "MUST NOT use Downloads URI below API 29 — MediaStore.Downloads requires Q",
            useDownloadsUriForSdk(Build.VERSION_CODES.P),
        )
    }

    // -------------------------------------------------------------------------
    // Tests 4-6: Persistable URI permission flags
    // -------------------------------------------------------------------------

    @Test
    fun `PERSISTABLE_URI_FLAGS includes read grant`() {
        assertNotEquals(
            "Persistable flags MUST include FLAG_GRANT_READ_URI_PERMISSION",
            0,
            PERSISTABLE_URI_FLAGS and Intent.FLAG_GRANT_READ_URI_PERMISSION,
        )
    }

    @Test
    fun `PERSISTABLE_URI_FLAGS includes write grant`() {
        // Secondary fix from the same debug session: OpenDocumentTree grants both
        // read+write; the original code only took read, which some providers
        // (including GrapheneOS) treat as an incomplete grant. Dropping the write
        // flag is a regression.
        assertNotEquals(
            "Persistable flags MUST include FLAG_GRANT_WRITE_URI_PERMISSION " +
                "— see .planning/debug/resolved/android-saf-cant-use-folder-grapheneos.md",
            0,
            PERSISTABLE_URI_FLAGS and Intent.FLAG_GRANT_WRITE_URI_PERMISSION,
        )
    }

    @Test
    fun `PERSISTABLE_URI_FLAGS equals read OR write exactly`() {
        // Pin the exact bitmask so any future addition or removal is an explicit
        // decision visible in code review.
        assertEquals(
            Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION,
            PERSISTABLE_URI_FLAGS,
        )
    }

    // -------------------------------------------------------------------------
    // Test 7: Lint contract on the SDK gate (release remediation finding 5)
    // -------------------------------------------------------------------------

    /**
     * `:app:lintDebug` must accept the `MediaStore.Downloads` (API 29) access in
     * `initialTreeUriForSdk`. That only holds while the helper carries
     * `@ChecksSdkIntAtLeast(api = Q)` — without it lint cannot recognize the
     * Boolean helper as an SDK-int guard and fails the release build with NewApi.
     *
     * `ChecksSdkIntAtLeast` has CLASS retention (not RUNTIME), so reflection cannot
     * see it; assert on the compiled `DirectorySourcePickerKt.class` bytes instead.
     * This keeps the lint contract a tested property instead of an incidental
     * detail someone can strip in a refactor.
     */
    @Test
    fun `useDownloadsUriForSdk is annotated as a lint-recognized SDK gate`() {
        val classBytes = javaClass.classLoader
            .getResourceAsStream("dev/disobey/mango/ui/DirectorySourcePickerKt.class")
            ?.use { it.readBytes() }
        assertNotNull(
            "Compiled DirectorySourcePickerKt.class must be on the unit-test classpath",
            classBytes,
        )
        // Constant-pool scan: CLASS-retention annotations survive in the bytecode.
        // The type descriptor appears iff some declaration in the file is annotated.
        val containsAnnotation = String(classBytes!!, Charsets.ISO_8859_1)
            .contains("Landroidx/annotation/ChecksSdkIntAtLeast;")
        assertTrue(
            "useDownloadsUriForSdk MUST carry @ChecksSdkIntAtLeast so Android lint " +
                "recognizes the API 29 gate in initialTreeUriForSdk (:app:lintDebug NewApi)",
            containsAnnotation,
        )
    }
}
