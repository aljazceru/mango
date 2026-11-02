package dev.disobey.mango

import dev.disobey.mango.rust.KeychainProvider

/** Auth-bound holder of the biometric DEK (see [BiometricDekStore]). */
interface BoundDekStore {
    fun wrap(dek: String): Boolean
    fun consumeAuthenticatedDek(): String?
    fun clear(): Boolean
}

/** Synchronous (commit-backed) string storage — EncryptedSharedPreferences in the app. */
interface PlainSecretStore {
    fun get(key: String): String?
    fun put(key: String, value: String): Boolean
    fun remove(key: String): Boolean
}

/**
 * Platform keychain for the Rust core. Every slot lives in [plain] except the
 * biometric DEK ("mango"/"dek"), which is only ever held wrapped by [bound]
 * and released solely to the read that follows a successful CryptoObject
 * authentication. A pre-split install's unauthenticated "mango::dek" copy is
 * served only until the core's one-time slot migration re-stores the slot,
 * which deletes it.
 */
class MangoKeychain(
    private val plain: PlainSecretStore,
    private val bound: BoundDekStore,
) : KeychainProvider {

    override fun store(service: String, key: String, value: String): Boolean {
        val id = "$service::$key"
        if (id == BIOMETRIC_DEK_ID) {
            return bound.wrap(value) && plain.remove(id)
        }
        return plain.put(id, value)
    }

    override fun load(service: String, key: String): String? {
        val id = "$service::$key"
        if (id == BIOMETRIC_DEK_ID) {
            return bound.consumeAuthenticatedDek() ?: plain.get(id)
        }
        return plain.get(id)
    }

    override fun delete(service: String, key: String): Boolean {
        val id = "$service::$key"
        if (id == BIOMETRIC_DEK_ID) {
            return bound.clear() && plain.remove(id)
        }
        return plain.remove(id)
    }

    companion object {
        const val BIOMETRIC_DEK_ID = "mango::dek"
        /** Non-secret enrollment marker written by the core (KEYCHAIN_BIOMETRIC_FLAG_KEY). */
        const val BIOMETRIC_FLAG_ID = "mango::biometric-enabled"
    }
}
