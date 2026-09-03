package dev.disobey.mango

import dev.disobey.mango.ui.ppq.formatPpqBalance
import org.junit.Assert.assertEquals
import org.junit.Test

class PpqBalanceFormatTest {
    @Test
    fun rounds_float_noise_to_four_digits() {
        assertEquals("0.9238", formatPpqBalance("0.9237520278000001"))
        assertEquals("0.1666", formatPpqBalance("0.16660347199999997"))
    }

    @Test
    fun trims_zeros_and_handles_simple_values() {
        assertEquals("1", formatPpqBalance("1.0000"))
        assertEquals("0.92", formatPpqBalance("0.9200"))
        assertEquals("0", formatPpqBalance("0"))
        assertEquals("12.5", formatPpqBalance("12.50000"))
    }

    @Test
    fun rounds_up_with_carry() {
        assertEquals("1", formatPpqBalance("0.99999"))
        assertEquals("2", formatPpqBalance("1.99995"))
    }

    @Test
    fun handles_missing_balance() {
        assertEquals("—", formatPpqBalance(null))
        assertEquals("—", formatPpqBalance(""))
    }
}
