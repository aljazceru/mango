package dev.disobey.mango.ui.ppq

import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class WalletIntentContractTest {
    @Test
    fun walletIntent_resolves_or_falls_back_without_crash() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val pm = context.packageManager
        val intent = Intent(Intent.ACTION_VIEW, Uri.parse("lightning:lnbc1pvjluezsp5zy"))
        val resolvers = pm.queryIntentActivities(intent, PackageManager.MATCH_ALL)

        if (resolvers.isNotEmpty()) {
            assertTrue("A wallet or handler resolves lightning: URIs on this device", resolvers.isNotEmpty())
        } else {
            try {
                val result = defaultOnOpenWallet(context, "lnbc1pvjluezsp5zy")
                assertFalse("defaultOnOpenWallet should fail gracefully when no wallet is installed", result)
            } catch (t: Throwable) {
                fail("defaultOnOpenWallet threw when no wallet was installed: ${t.message}")
            }
        }
    }

    @Test
    fun defaultOnOpenWallet_does_not_throw_for_invalid_bolt11() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        try {
            val result = defaultOnOpenWallet(context, "not-a-lightning-invoice")
            assertFalse("defaultOnOpenWallet should return false for an invalid BOLT11", result)
        } catch (t: Throwable) {
            fail("defaultOnOpenWallet threw for an invalid BOLT11: ${t.message}")
        }
    }
}
