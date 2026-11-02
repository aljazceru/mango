package dev.disobey.mango

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class MangoKeychainTest {

    private class MapStore : PlainSecretStore {
        val map = mutableMapOf<String, String>()
        override fun get(key: String) = map[key]
        override fun put(key: String, value: String): Boolean { map[key] = value; return true }
        override fun remove(key: String): Boolean { map.remove(key); return true }
    }

    /** Stands in for the Keystore wrapper: releases the DEK only after "auth". */
    private class FakeBound : BoundDekStore {
        var wrapped: String? = null
        var authenticated = false
        override fun wrap(dek: String): Boolean { wrapped = dek; return true }
        override fun consumeAuthenticatedDek(): String? =
            wrapped?.takeIf { authenticated }.also { authenticated = false }
        override fun clear(): Boolean { wrapped = null; return true }
    }

    @Test
    fun biometricDekIsNeverReadableBeforeAuthentication() {
        val plain = MapStore()
        val bound = FakeBound()
        val kc = MangoKeychain(plain, bound)

        assertTrue(kc.store("mango", "dek", "deadbeef"))

        assertNull("no unauthenticated plaintext copy", plain.map[MangoKeychain.BIOMETRIC_DEK_ID])
        assertNull("cold-start load before auth", kc.load("mango", "dek"))
        bound.authenticated = true
        assertEquals("deadbeef", kc.load("mango", "dek"))
        assertNull("release is one-shot", kc.load("mango", "dek"))
    }

    @Test
    fun legacyPlaintextCopyIsRemovedWhenTheCoreReStoresTheSlot() {
        val plain = MapStore().apply { map[MangoKeychain.BIOMETRIC_DEK_ID] = "legacy" }
        val bound = FakeBound()
        val kc = MangoKeychain(plain, bound)

        // The core's one-time migration reads the legacy copy, then re-stores it.
        assertEquals("legacy", kc.load("mango", "dek"))
        kc.store("mango", "dek", "legacy")

        assertNull(plain.map[MangoKeychain.BIOMETRIC_DEK_ID])
        assertNull(kc.load("mango", "dek"))
    }

    @Test
    fun otherSlotsUsePlainStorage() {
        val plain = MapStore()
        val kc = MangoKeychain(plain, FakeBound())
        kc.store("mango", "dek-bypass", "cafe")
        assertEquals("cafe", kc.load("mango", "dek-bypass"))
        kc.delete("mango", "dek-bypass")
        assertNull(kc.load("mango", "dek-bypass"))
    }
}
