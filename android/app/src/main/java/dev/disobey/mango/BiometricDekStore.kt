package dev.disobey.mango

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyPermanentlyInvalidatedException
import android.security.keystore.KeyProperties
import android.security.keystore.StrongBoxUnavailableException
import androidx.biometric.BiometricPrompt
import java.security.KeyFactory
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.security.PrivateKey
import java.security.PublicKey
import java.security.spec.MGF1ParameterSpec
import java.security.spec.X509EncodedKeySpec
import javax.crypto.Cipher
import javax.crypto.spec.OAEPParameterSpec
import javax.crypto.spec.PSource

/**
 * Authentication-bound storage for the biometric-unlock DEK (keychain slot
 * service "mango", key "dek").
 *
 * The DEK is wrapped under the public half of an AndroidKeyStore RSA keypair:
 *
 *   - Public-key encryption is exempt from `setUserAuthenticationRequired`, so
 *     the core can store/re-store the slot synchronously, without a prompt.
 *   - The private key requires fresh BIOMETRIC_STRONG authentication per use
 *     and is only usable through `BiometricPrompt.CryptoObject` (no boolean
 *     gate).
 *   - `setInvalidatedByBiometricEnrollment(true)`: enrolling a new fingerprint
 *     permanently invalidates the key, so an attacker who adds a device
 *     fingerprint cannot unlock Mango; the user falls back to the app PIN.
 *
 * No unauthenticated copy of the DEK is kept here. The Never/no-lock bypass
 * uses its own keychain slot ("dek-bypass") that exists only in those modes.
 */
class BiometricDekStore(
    context: Context,
    /** Invoked when the wrapping key died (enrollment change) and the slot was cleared. */
    private val onKeyInvalidated: () -> Unit = {},
) : BoundDekStore {

    private val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)

    @Volatile
    private var authenticatedDek: String? = null

    override fun wrap(dek: String): Boolean = try {
        val cipher = Cipher.getInstance(TRANSFORMATION).apply {
            init(Cipher.ENCRYPT_MODE, usablePublicKey(), OAEP_SPEC)
        }
        val blob = cipher.doFinal(dek.toByteArray(Charsets.UTF_8))
        authenticatedDek = null
        prefs.edit().putString(KEY_BLOB, blob.toHex()).commit()
    } catch (_: Exception) {
        false
    }

    /**
     * Decrypt cipher for the unlock prompt. Throws when nothing is wrapped or
     * the key is gone; an invalidated key also clears the slot.
     */
    fun unlockCryptoObject(): BiometricPrompt.CryptoObject {
        checkNotNull(readBlob()) { "no wrapped DEK" }
        val privateKey = keyStore().getKey(KEY_ALIAS, null) as? PrivateKey
            ?: throw IllegalStateException("biometric wrapping key missing")
        val cipher = Cipher.getInstance(TRANSFORMATION)
        try {
            cipher.init(Cipher.DECRYPT_MODE, privateKey, OAEP_SPEC)
        } catch (e: KeyPermanentlyInvalidatedException) {
            clear()
            onKeyInvalidated()
            throw e
        }
        return BiometricPrompt.CryptoObject(cipher)
    }

    /** Unwrap with the prompt-authenticated cipher into the one-shot channel. */
    fun onAuthenticationSucceeded(cryptoObject: BiometricPrompt.CryptoObject?): Boolean {
        val cipher = cryptoObject?.cipher ?: return false
        val blob = readBlob() ?: return false
        return try {
            authenticatedDek = String(cipher.doFinal(blob), Charsets.UTF_8)
            true
        } catch (_: Exception) {
            false
        }
    }

    /** One-shot: the keychain read following a successful bound authentication. */
    override fun consumeAuthenticatedDek(): String? {
        val value = authenticatedDek
        authenticatedDek = null
        return value
    }

    override fun clear(): Boolean {
        authenticatedDek = null
        try {
            keyStore().deleteEntry(KEY_ALIAS)
        } catch (_: Exception) {
        }
        return prefs.edit().remove(KEY_BLOB).commit()
    }

    private fun readBlob(): ByteArray? = try {
        prefs.getString(KEY_BLOB, null)?.hexToBytes()
    } catch (_: Exception) {
        null
    }

    private fun keyStore(): KeyStore =
        KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }

    /**
     * Public key of a still-valid keypair, generating a fresh one when absent or
     * invalidated. Only reached from store(), i.e. an unlocked, user-initiated
     * enrollment — never from the lock screen.
     */
    private fun usablePublicKey(): PublicKey {
        val ks = keyStore()
        val existing = ks.getKey(KEY_ALIAS, null) as? PrivateKey
        if (existing != null) {
            val valid = try {
                // Per-use auth keys init without a prompt; invalidated keys throw here.
                Cipher.getInstance(TRANSFORMATION).init(Cipher.DECRYPT_MODE, existing, OAEP_SPEC)
                true
            } catch (_: KeyPermanentlyInvalidatedException) {
                false
            }
            if (valid) return softwarePublicKey(ks.getCertificate(KEY_ALIAS).publicKey)
            ks.deleteEntry(KEY_ALIAS)
        }
        val generator = KeyPairGenerator.getInstance(KeyProperties.KEY_ALGORITHM_RSA, ANDROID_KEYSTORE)
        val pair = try {
            generator.initialize(keySpec(strongBox = true))
            generator.generateKeyPair()
        } catch (_: StrongBoxUnavailableException) {
            generator.initialize(keySpec(strongBox = false))
            generator.generateKeyPair()
        }
        return softwarePublicKey(pair.public)
    }

    // The keystore's public key object would route encryption through the
    // keystore provider; a plain copy lets the default provider encrypt.
    private fun softwarePublicKey(key: PublicKey): PublicKey =
        KeyFactory.getInstance(key.algorithm).generatePublic(X509EncodedKeySpec(key.encoded))

    private fun keySpec(strongBox: Boolean): KeyGenParameterSpec =
        KeyGenParameterSpec.Builder(
            KEY_ALIAS,
            KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
        )
            .setKeySize(KEY_SIZE_BITS)
            .setDigests(KeyProperties.DIGEST_SHA256)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_RSA_OAEP)
            .setUserAuthenticationRequired(true)
            .setInvalidatedByBiometricEnrollment(true)
            .apply { if (strongBox) setIsStrongBoxBacked(true) }
            .build()

    private fun ByteArray.toHex(): String =
        joinToString("") { "%02x".format(it) }

    private fun String.hexToBytes(): ByteArray =
        chunked(2).map { it.toInt(16).toByte() }.toByteArray()

    companion object {
        private const val ANDROID_KEYSTORE = "AndroidKeyStore"
        // New alias: the earlier AES wrapping key is not reused.
        private const val KEY_ALIAS = "mango_biometric_dek_rsa"
        private const val PREFS_NAME = "mango_biometric_dek"
        private const val KEY_BLOB = "rsa_blob"
        private const val TRANSFORMATION = "RSA/ECB/OAEPWithSHA-256AndMGF1Padding"
        private const val KEY_SIZE_BITS = 2048

        // Keystore OAEP supports MGF1 only with SHA-1 on API < 34.
        private val OAEP_SPEC = OAEPParameterSpec(
            "SHA-256",
            "MGF1",
            MGF1ParameterSpec.SHA1,
            PSource.PSpecified.DEFAULT,
        )
    }
}
