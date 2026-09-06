package dev.disobey.mango

import android.content.Context
import android.os.Handler
import android.os.Looper
import androidx.fragment.app.FragmentActivity
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.neverEqualPolicy
import androidx.compose.runtime.setValue
import dev.disobey.mango.rust.AppAction
import dev.disobey.mango.rust.AppReconciler
import dev.disobey.mango.rust.AppState
import dev.disobey.mango.rust.PpqAccountMode
import dev.disobey.mango.rust.PpqAccountSummary
import dev.disobey.mango.rust.PpqFundingPhase
import dev.disobey.mango.rust.PpqSetupPhase
import dev.disobey.mango.rust.AppUpdate
import dev.disobey.mango.rust.BiometricProvider
import dev.disobey.mango.rust.BusyState
import dev.disobey.mango.rust.ContextvmDiscoveryState
import dev.disobey.mango.rust.DeviceCapability
import dev.disobey.mango.rust.DirectoryFingerprint
import dev.disobey.mango.rust.EmbeddingProvider
import dev.disobey.mango.rust.EmbeddingStatus
import dev.disobey.mango.rust.FfiApp
import dev.disobey.mango.rust.KeychainProvider
import dev.disobey.mango.rust.OnboardingState
import dev.disobey.mango.rust.AttestationStatus
import dev.disobey.mango.rust.LocalLlmCapabilityStatus
import dev.disobey.mango.rust.Router
import dev.disobey.mango.rust.Screen
import dev.disobey.mango.rust.SensitiveActionAuth
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.withContext

class AppManager private constructor(context: Context, activity: FragmentActivity?) : AppReconciler {
    /** §7.5: activity-rebindable biometric bridge injected into Rust. */
    private lateinit var biometricProxy: RebindableBiometricProvider
    val biometricRebindable: RebindableBiometricProvider? get() = if (::biometricProxy.isInitialized) biometricProxy else null

    private val mainHandler = Handler(Looper.getMainLooper())
    private val ffiApp: FfiApp
    private var lastRevApplied: ULong = 0UL

    private val _stateFlow: MutableStateFlow<AppState>
    val stateFlow: StateFlow<AppState> get() = _stateFlow.asStateFlow()

    // True once the first reconcile() call from the Rust actor has arrived.
    // Prevents the UI from rendering the hardcoded default state (Screen.Home) before
    // the actor has finished DB init and determined the real initial screen (e.g. Onboarding).
    var isReady: Boolean by mutableStateOf(false)
        private set

    var state: AppState by mutableStateOf(
        value = AppState(
            rev = 0UL,
            router = Router(
                currentScreen = Screen.Home,
                screenStack = emptyList(),
            ),
            busyState = BusyState.Idle,
            toast = null,
            backends = emptyList(),
            activeBackendId = null,
            streamingText = null,
            lastError = null,
            attestationStatuses = emptyList(),
            conversations = emptyList(),
            agentSessions = emptyList(),
            currentConversationId = null,
            messages = emptyList(),
            pendingAttachment = null,
            onboarding = OnboardingState(
                selectedBackendId = null,
                attestationStage = null,
                attestationResult = null,
                attestationTeeLabel = null,
                validatingApiKey = false,
                apiKeyError = null,
            ),
            showFirstChatPlaceholder = false,
            documents = emptyList(),
            ingestionProgress = null,
            currentConversationAttachedDocs = emptyList(),
            currentAgentSessionId = null,
            currentAgentSteps = emptyList(),
            attestationIntervalMinutes = 30u,
            embeddingStatus = EmbeddingStatus.ACTIVE,
            localDeviceCapability = DeviceCapability(
                abi = "initializing",
                totalRamBytes = 0UL,
                availableRamBytes = 0UL,
                supportsMmap = false,
                status = LocalLlmCapabilityStatus.UNKNOWN,
                reasonCode = "unknown",
                reason = "initializing",
                availableStorageBytes = 0UL,
            ),
            localModels = emptyList(),
            localDownloadProgress = null,
            localInferenceEnabled = true,
            globalSystemPrompt = null,
            defaultModelId = null,
            memories = emptyList(),
            memoryCount = 0UL,
            braveApiKeySet = false,
            braveApiKeyValidating = false,
            memoriesEnabled = true,
            biometricAvailable = false,
            biometricLoginEnabled = false,
            biometricAuthenticated = false,
            duressPinConfigured = false,
            lockTimeoutSeconds = 300L,
            authInitialized = false,
            encryptionEnabled = false,
            directorySources = emptyList(),
            contextvmTools = emptyList(),
            autoDiscoverToolsEnabled = false,
            contextvmDiscoveryState = ContextvmDiscoveryState.Idle,
            hybridProfiles = emptyList(),
            lastTurnRouting = null,
            trustedProviders = emptyList(),
            enrollmentResumePending = false,
            ppq = PpqAccountSummary(
                mode = PpqAccountMode.NONE,
                setupPhase = PpqSetupPhase.IDLE,
                fundingPhase = PpqFundingPhase.IDLE,
                balanceDisplay = null,
                balanceUpdatedAt = null,
                backupConfirmed = false,
                firstFundingReminderShown = false,
                funding = null,
                error = null,
                destructivePreflight = null,
            ),
        ),
        policy = neverEqualPolicy(),
    )
        private set

    init {
        val dataDir = context.filesDir.absolutePath
        val masterKey = MasterKey.Builder(context)
            .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
            .build()
        val prefs = EncryptedSharedPreferences.create(
            context,
            "keychain_encrypted",
            masterKey,
            EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
            EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM,
        )
        val keychain = object : KeychainProvider {
            override fun store(service: String, key: String, value: String): Boolean {
                // commit() (synchronous) per plan §6.2: the Rust secret store relies on
                // store() returning success only after the write would survive process
                // death. apply() is async and can silently lose the credential.
                return prefs.edit().putString("$service::$key", value).commit()
            }
            override fun load(service: String, key: String): String? {
                return prefs.getString("$service::$key", null)
            }
            override fun delete(service: String, key: String): Boolean {
                return prefs.edit().remove("$service::$key").commit()
            }
        }
        // Real on-device embedding via ONNX Runtime + XNNPACK EP (Phase 11, EMBD-03/05).
        // Falls back to a zero-vector provider if model initialisation fails so the app
        // remains functional even on devices where ONNX Runtime cannot load the model.
        val embeddingResult: Pair<EmbeddingProvider, EmbeddingStatus> = try {
            Pair(MobileEmbeddingProvider(context), EmbeddingStatus.ACTIVE)
        } catch (e: Exception) {
            android.util.Log.e(
                "AppManager",
                "MobileEmbeddingProvider init failed: ${e.message}, falling back to null provider"
            )
            // Fallback: zero-vector provider — non-semantic but does not crash
            val fallback = object : EmbeddingProvider {
                override fun embed(texts: List<String>): List<Float> {
                    return List(texts.size * 384) { 0.0f }
                }
            }
            Pair(fallback, EmbeddingStatus.DEGRADED)
        }
        val embedding = embeddingResult.first
        val embeddingStatus = embeddingResult.second
        val localLlm = AndroidLocalLlmProvider(context)
        // §7.5: inject the rebindable proxy — MainActivity re-attaches the
        // current activity on every onCreate, so rotation/recreation can never
        // leave Rust holding a stale weak activity reference. The initial
        // activity (if any) is attached immediately here.
        val biometricProxy = RebindableBiometricProvider()
        activity?.let { biometricProxy.attach(it) }
        val biometric: BiometricProvider = biometricProxy
        this.biometricProxy = biometricProxy
        ffiApp = FfiApp(dataDir, keychain, embedding, embeddingStatus, localLlm, biometric)
        val initial = ffiApp.state()
        state = initial
        lastRevApplied = initial.rev
        _stateFlow = MutableStateFlow(initial)
        ffiApp.listenForUpdates(this)
    }

    fun dispatch(action: AppAction) {
        ffiApp.dispatch(action)
    }

    // IMG-07: decrypt-on-read for encrypted image thumbnails
    fun readEncryptedImage(messageId: String): ByteArray {
        return ffiApp.readEncryptedImage(messageId)
    }

    fun exportConversationMarkdown(conversationId: String): String {
        return ffiApp.exportConversationMarkdown(conversationId)
    }

    // Phase 32 Plan 06: native-side diff needs stored fingerprints so the
    // Android sync pipeline can determine added/modified/removed without
    // crossing the persistence/SAF permission boundary (T-32-I2 / D-02).
    fun listDirectoryFingerprints(sourceId: String): List<DirectoryFingerprint> {
        return ffiApp.listDirectoryFingerprints(sourceId)
    }

    /**
     * Create an encrypted PPQ recovery backup. Returns the encrypted bytes on success,
     * null on failure. Never logs secrets (backup password, PIN, or bytes).
     */
    suspend fun createPpqRecoveryBackup(
        backupPassword: String,
        useBiometric: Boolean,
        pin: String?,
    ): ByteArray? = withContext(Dispatchers.IO) {
        val auth = when {
            useBiometric -> SensitiveActionAuth.Biometric
            pin != null -> SensitiveActionAuth.MainPin(pin = pin)
            else -> {
                android.util.Log.e("AppManager", "createPpqRecoveryBackup: no auth available")
                return@withContext null
            }
        }
        try {
            ffiApp.createPpqRecoveryBackup(backupPassword, auth)
        } catch (e: Exception) {
            android.util.Log.e("AppManager", "createPpqRecoveryBackup failed", e)
            null
        }
    }

    /**
     * Restore a PPQ account from an encrypted recovery backup. Returns true on success.
     * Never logs secrets.
     */
    suspend fun restorePpqRecoveryBackup(
        bytes: ByteArray,
        backupPassword: String,
        useBiometric: Boolean,
        pin: String?,
        replaceAcknowledged: Boolean = false,
    ): Boolean = withContext(Dispatchers.IO) {
        val auth = when {
            useBiometric -> SensitiveActionAuth.Biometric
            pin != null -> SensitiveActionAuth.MainPin(pin = pin)
            else -> {
                android.util.Log.e("AppManager", "restorePpqRecoveryBackup: no auth available")
                return@withContext false
            }
        }
        try {
            ffiApp.restorePpqRecoveryBackup(bytes, backupPassword, auth, replaceAcknowledged).success
        } catch (e: Exception) {
            android.util.Log.e("AppManager", "restorePpqRecoveryBackup failed", e)
            false
        }
    }

    override fun reconcile(update: AppUpdate) {
        mainHandler.post {
            when (update) {
                is AppUpdate.FullState -> {
                    // Use strict < so the initial rev=0 emit is never skipped when
                    // ffiApp.state() already captured the same rev (Case D race).
                    if (update.v1.rev < lastRevApplied) return@post
                    lastRevApplied = update.v1.rev
                    state = update.v1
                    _stateFlow.value = update.v1
                    // Mark ready on the first reconcile so the UI renders the real
                    // initial state rather than the hardcoded Screen.Home default.
                    if (!isReady) isReady = true
                }
            }
        }
    }

    companion object {
        @Volatile
        private var instance: AppManager? = null

        fun getInstance(context: Context, activity: FragmentActivity? = null): AppManager =
            instance ?: synchronized(this) {
                instance ?: AppManager(context.applicationContext, activity).also { instance = it }
            }
    }
}
