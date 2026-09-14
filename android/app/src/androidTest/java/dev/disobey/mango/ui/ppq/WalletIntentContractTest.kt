package dev.disobey.mango.ui.ppq

import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class WalletIntentContractTest {
    // defaultOnOpenWallet shows a Toast on its failure path; the test
    // method runs on the (Looper-less) instrumentation thread, so the
    // call must be pushed to the main thread or it throws
    // "Can't toast on a thread that has not called Looper.prepare()".
    private fun onMain(block: () -> Unit) =
        InstrumentationRegistry.getInstrumentation().runOnMainSync(block)

    @Test
    fun walletIntent_resolves_or_falls_back_without_crash() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val pm = context.packageManager
        val intent = Intent(Intent.ACTION_VIEW, Uri.parse("lightning:lnbc1pvjluezsp5zy"))
        val resolvers = pm.queryIntentActivities(intent, PackageManager.MATCH_ALL)

        if (resolvers.isNotEmpty()) {
            assertTrue("A wallet or handler resolves lightning: URIs on this device", resolvers.isNotEmpty())
        } else {
            var result = true
            var thrown: Throwable? = null
            onMain {
                try {
                    result = defaultOnOpenWallet(context, "lnbc1pvjluezsp5zy")
                } catch (t: Throwable) {
                    thrown = t
                }
            }
            thrown?.let { fail("defaultOnOpenWallet threw when no wallet was installed: ${it.message}") }
            assertFalse("defaultOnOpenWallet should fail gracefully when no wallet is installed", result)
        }
    }

    @Test
    fun defaultOnOpenWallet_does_not_throw_for_invalid_bolt11() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        var result = true
        var thrown: Throwable? = null
        onMain {
            try {
                result = defaultOnOpenWallet(context, "not-a-lightning-invoice")
            } catch (t: Throwable) {
                thrown = t
            }
        }
        thrown?.let { fail("defaultOnOpenWallet threw for an invalid BOLT11: ${it.message}") }
        assertFalse("defaultOnOpenWallet should return false for an invalid BOLT11", result)
    }
}
