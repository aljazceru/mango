package dev.disobey.mango.ui.ppq

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class PpqUiLogicInstrumentedTest {
    @Test
    fun buildLightningUri_accepts_lnbc_and_lntb() {
        assertEquals("lightning:lnbc1pvjluezsp5zy", buildLightningUri("lnbc1pvjluezsp5zy"))
        assertEquals("lightning:lntb1pvjluezsp5zy", buildLightningUri("lntb1pvjluezsp5zy"))
        assertEquals("lightning:lnbc1pvjluezsp5zy", buildLightningUri("  lnbc1pvjluezsp5zy  "))
    }

    @Test
    fun buildLightningUri_rejects_non_lightning_uris() {
        assertNull(buildLightningUri("bitcoin:abc"))
        assertNull(buildLightningUri("https://example.com"))
        assertNull(buildLightningUri("lightning:lnbc1pvjluezsp5zy"))
        assertNull(buildLightningUri("LNBC1PVJLUEZSP5ZY"))
    }

    @Test
    fun buildLightningUri_rejects_blank_and_whitespace() {
        assertNull(buildLightningUri(""))
        assertNull(buildLightningUri("   "))
        assertNull(buildLightningUri("lnbc 1 pvjluez"))
        assertNull(buildLightningUri("lnbc1pvjluez\n"))
        assertNull(buildLightningUri("lnbc1pvjluez\t"))
    }

    @Test
    fun formatPpqBalance_rounds_and_trims() {
        assertEquals("0.9238", formatPpqBalance("0.9237520278000001"))
        assertEquals("0.1666", formatPpqBalance("0.16660347199999997"))
        assertEquals("1", formatPpqBalance("1.0000"))
        assertEquals("0.92", formatPpqBalance("0.9200"))
        assertEquals("—", formatPpqBalance(null))
        assertEquals("—", formatPpqBalance(""))
        assertEquals("0.9999", formatPpqBalance("0.99994"))
        assertEquals("1", formatPpqBalance("0.99995"))
        assertEquals("2", formatPpqBalance("1.99995"))
    }
}
