package dev.disobey.mango.ui.ppq

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class PpqUiLogicTest {
    @Test
    fun buildLightningUri_table() {
        // Accepted mainnet and testnet BOLT11 invoices.
        assertEquals("lightning:lnbc1pvjluezsp5zy", buildLightningUri("lnbc1pvjluezsp5zy"))
        assertEquals("lightning:lntb1pvjluezsp5zy", buildLightningUri("lntb1pvjluezsp5zy"))
        assertEquals("lightning:lnbc1pvjluezsp5zy", buildLightningUri("  lnbc1pvjluezsp5zy  "))

        // Rejected: wrong scheme, already-schemed, uppercase, blank/empty, whitespace.
        assertNull(buildLightningUri(""))
        assertNull(buildLightningUri("   "))
        assertNull(buildLightningUri("bitcoin:abc"))
        assertNull(buildLightningUri("https://example.com"))
        assertNull(buildLightningUri("lightning:lnbc1pvjluezsp5zy"))
        assertNull(buildLightningUri("LNBC1PVJLUEZSP5ZY"))
        assertNull(buildLightningUri("lnbc 1 pvjluez"))
        assertNull(buildLightningUri("lnbc1pvjluez\n"))
        assertNull(buildLightningUri("lnbc1pvjluez\t"))
    }

    @Test
    fun formatPpqBalance_table() {
        assertEquals("0.9238", formatPpqBalance("0.9237520278000001"))
        assertEquals("0.1666", formatPpqBalance("0.16660347199999997"))
        assertEquals("1", formatPpqBalance("1.0000"))
        assertEquals("0.92", formatPpqBalance("0.9200"))
        assertEquals("12.5", formatPpqBalance("12.50000"))
        assertEquals("0.9999", formatPpqBalance("0.99994"))
        assertEquals("1", formatPpqBalance("0.99995"))
        assertEquals("2", formatPpqBalance("1.99995"))
        assertEquals("0", formatPpqBalance("0"))
        assertEquals("—", formatPpqBalance(null))
        assertEquals("—", formatPpqBalance(""))
        assertEquals("—", formatPpqBalance("   "))
    }

    @Test
    fun parseSatsAmount_table() {
        assertEquals(1uL, parseSatsAmount("1"))
        assertEquals(2100uL, parseSatsAmount("2100"))
        assertEquals(5uL, parseSatsAmount("  5  "))
        assertEquals(100uL, parseSatsAmount("100", minSats = 50uL, maxSats = 150uL))

        assertNull(parseSatsAmount("0"))
        assertNull(parseSatsAmount(""))
        assertNull(parseSatsAmount("   "))
        assertNull(parseSatsAmount("-1"))
        assertNull(parseSatsAmount("abc"))
        assertNull(parseSatsAmount("12.3"))
        assertNull(parseSatsAmount("5 6"))
        assertNull(parseSatsAmount("6", minSats = 10uL))
        assertNull(parseSatsAmount("100", maxSats = 50uL))
    }
}
