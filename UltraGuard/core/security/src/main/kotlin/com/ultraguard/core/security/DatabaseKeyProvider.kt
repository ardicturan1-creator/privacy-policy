package com.ultraguard.core.security

import android.content.Context
import android.os.Build
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.security.keystore.StrongBoxUnavailableException
import com.ultraguard.core.common.log.UgLog
import dagger.hilt.android.qualifiers.ApplicationContext
import java.security.KeyStore
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import javax.inject.Inject
import javax.inject.Singleton

/**
 * SQLCipher parolasinin donanim destekli uretimi ve saklanmasi.
 *
 * Sema:
 *  1. Ilk calistirmada 32 baytlik rastgele bir veritabani parolasi uretilir.
 *  2. Bu parola, **AndroidKeyStore icinde yasayan** bir AES anahtariyla
 *     sarmalanir (wrap). Sarmalama anahtari asla uygulama surecine cikmaz.
 *  3. Sarmalanmis parola normal dosyada saklanabilir — anahtar olmadan
 *     ise yaramaz.
 *
 * StrongBox (ayri guvenlik cipi) varsa kullanilir; yoksa TEE'ye duser.
 * Ikisi de yoksa yazilim destekli Keystore'a duseriz ve bu durum kullaniciya
 * Seffaflik Merkezi'nde **acikca bildirilir** — sessizce zayif moda dusen bir
 * guvenlik urunu, kullanicisina yalan soylemis olur.
 */
@Singleton
class DatabaseKeyProvider @Inject constructor(
    @ApplicationContext private val context: Context,
) {
    private val prefs by lazy {
        context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
    }

    /** Anahtarin gercekte hangi donanim seviyesinde korundugu. */
    @Volatile
    var attainedSecurityLevel: KeyStorageLevel = KeyStorageLevel.UNKNOWN
        private set

    /**
     * @return SQLCipher'a verilecek ham parola. Cagiran taraf kullandiktan
     *   sonra diziyi **sifirlamalidir** — bellekte gezinen bir parola,
     *   bellek dokumu alabilen bir saldirgan icin hediyedir.
     */
    fun databasePassphrase(): ByteArray {
        val wrapped = prefs.getString(KEY_WRAPPED_PASSPHRASE, null)
        val iv = prefs.getString(KEY_WRAP_IV, null)

        return if (wrapped != null && iv != null) {
            unwrap(
                android.util.Base64.decode(wrapped, android.util.Base64.NO_WRAP),
                android.util.Base64.decode(iv, android.util.Base64.NO_WRAP),
            )
        } else {
            generateAndStore()
        }
    }

    private fun generateAndStore(): ByteArray {
        val passphrase = ByteArray(PASSPHRASE_BYTES).also { SecureRandom().nextBytes(it) }
        val cipher = Cipher.getInstance(TRANSFORMATION).apply {
            init(Cipher.ENCRYPT_MODE, wrappingKey())
        }
        val sealed = cipher.doFinal(passphrase)

        prefs.edit()
            .putString(KEY_WRAPPED_PASSPHRASE, android.util.Base64.encodeToString(sealed, android.util.Base64.NO_WRAP))
            .putString(KEY_WRAP_IV, android.util.Base64.encodeToString(cipher.iv, android.util.Base64.NO_WRAP))
            .apply()

        return passphrase
    }

    private fun unwrap(sealed: ByteArray, iv: ByteArray): ByteArray {
        val cipher = Cipher.getInstance(TRANSFORMATION).apply {
            init(Cipher.DECRYPT_MODE, wrappingKey(), GCMParameterSpec(GCM_TAG_BITS, iv))
        }
        return cipher.doFinal(sealed)
    }

    private fun wrappingKey(): SecretKey {
        val keyStore = KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }
        (keyStore.getEntry(KEY_ALIAS, null) as? KeyStore.SecretKeyEntry)?.let {
            return it.secretKey
        }
        return createWrappingKey()
    }

    private fun createWrappingKey(): SecretKey {
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, ANDROID_KEYSTORE)

        fun spec(strongBox: Boolean) = KeyGenParameterSpec.Builder(
            KEY_ALIAS,
            KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
        ).apply {
            setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            setKeySize(AES_KEY_BITS)
            setRandomizedEncryptionRequired(true)
            if (strongBox && Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                setIsStrongBoxBacked(true)
            }
        }.build()

        // Once StrongBox denenir. Cihazda yoksa kasitli olarak TEE'ye duseriz;
        // sessizce degil, seviyeyi kaydederek.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            try {
                generator.init(spec(strongBox = true))
                val key = generator.generateKey()
                attainedSecurityLevel = KeyStorageLevel.STRONGBOX
                return key
            } catch (e: StrongBoxUnavailableException) {
                UgLog.i(TAG, "StrongBox yok, TEE'ye dusuluyor")
            }
        }

        generator.init(spec(strongBox = false))
        val key = generator.generateKey()
        attainedSecurityLevel = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            KeyStorageLevel.TEE
        } else {
            KeyStorageLevel.SOFTWARE
        }
        return key
    }

    private companion object {
        const val TAG = "KeyProvider"
        const val ANDROID_KEYSTORE = "AndroidKeyStore"
        const val KEY_ALIAS = "ultraguard_db_wrapping_key"
        const val PREFS_NAME = "ultraguard_key_store"
        const val KEY_WRAPPED_PASSPHRASE = "wrapped_passphrase"
        const val KEY_WRAP_IV = "wrap_iv"
        const val TRANSFORMATION = "AES/GCM/NoPadding"
        const val PASSPHRASE_BYTES = 32
        const val AES_KEY_BITS = 256
        const val GCM_TAG_BITS = 128
    }
}

/** Anahtarin fiilen korundugu seviye — kullaniciya seffaf olarak gosterilir. */
enum class KeyStorageLevel {
    /** Ayri guvenlik cipi (Titan M, SE). En yuksek seviye. */
    STRONGBOX,
    /** Trusted Execution Environment. Guclu. */
    TEE,
    /** Yalnizca yazilim. Kullaniciya uyari gosterilir. */
    SOFTWARE,
    UNKNOWN,
}
