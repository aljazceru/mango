package dev.disobey.mango

import android.content.Context
import android.content.Intent
import android.os.SystemClock

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
import dev.disobey.mango.rust.InferenceRoute
import dev.disobey.mango.rust.InferenceStatus
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
import dev.disobey.mango.rust.ConversationRetentionMode
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
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * Lock-timeout predicate on elapsedRealtime-style monotonic values:
 * true when a finite timeout was armed (backgroundedAt > 0) and has elapsed,
 * or when the clock appears inverted (fail closed). timeoutSeconds < 0 = Never.
 */
internal fun shouldLockAfterBackground(backgroundedAt: Long, now: Long, timeoutSeconds: Long): Boolean {
    if (backgroundedAt <= 0 || timeoutSeconds < 0) {
        return false
    }
    val elapsed = now - backgroundedAt
    return elapsed < 0 || elapsed >= timeoutSeconds * 1000L
}

/**
 * Startup failed before any UI existed: EncryptedSharedPreferences / MasterKey /
 * Rust core construction threw. Surfaced to MainActivity so it can show a
 * retry/recovery screen instead of crashing; NOTHING is auto-cleared.
 */
class AppManagerInitException(message: String, cause: Throwable?) : Exception(message, cause)

class AppManager private constructor(context: Context, activity: FragmentActivity?) : AppReconciler {
    /** §7.5: activity-rebindable biometric bridge injected into Rust. */
    private lateinit var biometricProxy: RebindableBiometricProvider
    val biometricRebindable: RebindableBiometricProvider? get() = if (::biometricProxy.isInitialized) biometricProxy else null


    private val mainHandler = Handler(Looper.getMainLooper())
    private val ioScope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private lateinit var ffiApp: FfiApp
    private var lastRevApplied: ULong = 0UL

    private lateinit var _stateFlow: MutableStateFlow<AppState>
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
            noLockMode = false,
            conversationRetentionMode = ConversationRetentionMode.OFF,
            conversationRetentionDays = 30u,
            archivedConversations = emptyList(),
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
            localModelsError = null,
            compaction = null,
            compactionOffered = false,
            inferenceStatus = InferenceStatus(
                route = InferenceRoute.CLOUD,
                backendId = "",
                backendName = "",
                modelId = "",
                teeType = null,
                attestedBackendId = null,
                localModelName = null,
                hybridAvailable = false,
                onDeviceAvailable = false,
            ),

        ),
        policy = neverEqualPolicy(),
    )
        private set

    init {
        try {
            initCore(context, activity)
        } catch (e: Throwable) {
            throw AppManagerInitException(
                "Secure storage or core initialization failed",
                e,
            )
        }
    }

    private fun initCore(context: Context, activity: FragmentActivity?) {
        val dataDir = context.filesDir.absolutePath
        val masterKey = try {
            MasterKey.Builder(context)
                .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
                .build()
        } catch (e: Throwable) {
            throw AppManagerInitException("Keystore master key unavailable", e)
        }
        val prefs = try {
            EncryptedSharedPreferences.create(
                context,
                "keychain_encrypted",
                masterKey,
                EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
                EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM,
            )
        } catch (e: Throwable) {
            throw AppManagerInitException("Encrypted keychain unavailable", e)
        }
        // The biometric DEK slot is held only wrapped under an auth-bound
        // Keystore key (see BiometricDekStore / MangoKeychain); the Never /
        // no-lock bypass uses its own "dek-bypass" slot.
        val dekStore = BiometricDekStore(context) {
            // Wrapping key invalidated by an enrollment change: drop the
            // enrollment marker so the core stops offering biometric unlock.
            prefs.edit().remove(MangoKeychain.BIOMETRIC_FLAG_ID).commit()
        }
        val keychain = MangoKeychain(
            plain = object : PlainSecretStore {
                override fun get(key: String): String? = prefs.getString(key, null)
                // commit() (synchronous) per plan §6.2: the Rust secret store relies on
                // store() returning success only after the write would survive process
                // death. apply() is async and can silently lose the credential.
                override fun put(key: String, value: String): Boolean =
                    prefs.edit().putString(key, value).commit()
                override fun remove(key: String): Boolean = prefs.edit().remove(key).commit()
            },
            bound = dekStore,
        )
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
        val biometricProxy = RebindableBiometricProvider(context.applicationContext, dekStore)
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

    /** The exact persisted SAF tree URI for a source id (null when none/locked). */
    fun getDirectoryTreeUri(sourceId: String): String? = ffiApp.getDirectoryTreeUri(sourceId)

    /** FIFO barrier: blocks until the actor processed everything dispatched so far. */
    fun syncActor() = ffiApp.sync()

    // ── Lock deadline (process-wide, monotonic) ──────────────────────────────
    // The unlocked core lives in this process-wide singleton, so the
    // background timestamp must too — an activity-own timestamp resets to zero
    // on recreation and on back-to-launcher + relaunch, silently disabling the
    // timeout. SystemClock.elapsedRealtime is immune to wall-clock changes.
    private val backgroundLock = Any()
    /** elapsedRealtime (ms) when the app last went to background; 0 = none. */
    private var backgroundedAtElapsed: Long = 0L

    fun onAppBackgrounded() {
        synchronized(backgroundLock) {
            if (backgroundedAtElapsed == 0L) {
                backgroundedAtElapsed = SystemClock.elapsedRealtime()
            }
        }
    }

    /**
     * Enforce the configured lock timeout for ANY activity attach/resume:
     * returns true (and clears the marker) when the app must re-lock. The
     * marker is consumed either way so a stale deadline can never fire twice.
     */
    fun enforceLockDeadline(): Boolean {
        val marker: Long = synchronized(backgroundLock) {
            val m = backgroundedAtElapsed
            backgroundedAtElapsed = 0L
            m
        }
        return shouldLockAfterBackground(
            backgroundedAt = marker,
            now = SystemClock.elapsedRealtime(),
            timeoutSeconds = state.lockTimeoutSeconds,
        )
    }

    // ── Persisted SAF grant reconciliation ───────────────────────────────────
    private val appContext = context.applicationContext
    private var grantsReconciledKey: Set<String>? = null

    /**
     * Release persisted tree grants that no current directory source needs.
     * Runs after the actor reports a hydrated, UNLOCKED state — never while
     * locked or before hydration, so a pre-unlock snapshot can never revoke
     * grants a wiped/decoy session did not re-add.
     */
    fun maybeReconcileDirectoryGrants() {
        val s = state
        if (!isReady) return
        if (s.router.currentScreen is Screen.Locked) return
        val key = s.directorySources.map { it.id }.toSet()
        synchronized(this) {
            if (grantsReconciledKey == key) return
            grantsReconciledKey = key
        }
        val referenced = HashSet<String>(s.directorySources.size)
        try {
            for (source in s.directorySources) {
                val uri = ffiApp.getDirectoryTreeUri(source.id)
                // A locked/errored read would make reconciliation blind —
                // abort instead of revoking real grants.
                    ?: throw IllegalStateException("tree_uri lookup failed for ${source.id}")
                referenced += uri
            }
        } catch (e: Throwable) {
            android.util.Log.w("AppManager", "directory grant reconcile aborted: ${e.message}")
            synchronized(this) { grantsReconciledKey = null }
            return
        }
        if (state.rev != s.rev || state.router.currentScreen is Screen.Locked) {
            synchronized(this) { grantsReconciledKey = null }
            return
        }
        val resolver = appContext.contentResolver
        val grants = try {
            resolver.persistedUriPermissions
        } catch (_: SecurityException) {
            return
        }
        for (grant in grants) {
            val uriString = grant.uri.toString()
            if (uriString in referenced) continue
            try {
                resolver.releasePersistableUriPermission(
                    grant.uri,
                    Intent.FLAG_GRANT_READ_URI_PERMISSION or
                        Intent.FLAG_GRANT_WRITE_URI_PERMISSION,
                )
            } catch (e: Exception) {
                android.util.Log.w("AppManager", "releasePersistableUriPermission failed: ${e.message}")
            }
        }
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
     * Result of a PPQ recovery restore attempt. [errorCode] is Rust's
     * PpqRecoveryResult.errorCode (e.g. `replacement_confirmation_required`) so the
     * UI can offer an explicit replacement confirmation and retry. Secret-free.
     */
    data class PpqRestoreOutcome(
        val success: Boolean,
        val errorCode: String? = null,
    )

    data class PpqDestructiveOutcome(
        val success: Boolean,
        val error: String? = null,
    )

    private fun sensitiveAuth(useBiometric: Boolean, pin: String?): dev.disobey.mango.rust.SensitiveActionAuth? =
        when {
            useBiometric -> dev.disobey.mango.rust.SensitiveActionAuth.Biometric
            !pin.isNullOrBlank() -> dev.disobey.mango.rust.SensitiveActionAuth.MainPin(pin = pin)
            else -> null
        }

    /** Remove the managed PPQ account from this device (preflight-confirmed). */
    suspend fun forgetManagedPpq(
        useBiometric: Boolean,
        pin: String?,
        backupRiskAcknowledged: Boolean,
    ): PpqDestructiveOutcome = withContext(Dispatchers.IO) {
        val auth = sensitiveAuth(useBiometric, pin) ?: run {
            android.util.Log.e("AppManager", "forgetManagedPpq: no auth available")
            return@withContext PpqDestructiveOutcome(success = false)
        }
        try {
            val result = ffiApp.confirmForgetManagedPpq(auth, backupRiskAcknowledged)
            PpqDestructiveOutcome(result.success, result.error)
        } catch (e: Exception) {
            android.util.Log.e("AppManager", "forgetManagedPpq failed", e)
            PpqDestructiveOutcome(success = false, error = e.message)
        }
    }

    /** Delete all app data (preflight-confirmed). */
    suspend fun deleteAllData(
        useBiometric: Boolean,
        pin: String?,
        backupRiskAcknowledged: Boolean,
    ): PpqDestructiveOutcome = withContext(Dispatchers.IO) {
        val auth = sensitiveAuth(useBiometric, pin) ?: run {
            android.util.Log.e("AppManager", "deleteAllData: no auth available")
            return@withContext PpqDestructiveOutcome(success = false)
        }
        try {
            val result = ffiApp.confirmDeleteAllData(auth, backupRiskAcknowledged)
            PpqDestructiveOutcome(result.success, result.error)
        } catch (e: Exception) {
            android.util.Log.e("AppManager", "deleteAllData failed", e)
            PpqDestructiveOutcome(success = false, error = e.message)
        }
    }

    /**
     * Restore a PPQ account from an encrypted recovery backup. Returns a
     * [PpqRestoreOutcome]; `success` is true only when Rust committed the restore.
     * Never logs secrets.
     */
    suspend fun restorePpqRecoveryBackup(
        bytes: ByteArray,
        backupPassword: String,
        useBiometric: Boolean,
        pin: String?,
        replaceAcknowledged: Boolean = false,
    ): PpqRestoreOutcome = withContext(Dispatchers.IO) {
        val auth = when {
            useBiometric -> SensitiveActionAuth.Biometric
            pin != null -> SensitiveActionAuth.MainPin(pin = pin)
            else -> {
                android.util.Log.e("AppManager", "restorePpqRecoveryBackup: no auth available")
                return@withContext PpqRestoreOutcome(success = false)
            }
        }
        try {
            val result = ffiApp.restorePpqRecoveryBackup(bytes, backupPassword, auth, replaceAcknowledged)
            PpqRestoreOutcome(success = result.success, errorCode = result.errorCode)
        } catch (e: Exception) {
            android.util.Log.e("AppManager", "restorePpqRecoveryBackup failed", e)
            PpqRestoreOutcome(success = false)
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
                    // Reconcile persisted SAF grants against the freshly
                    // applied state off the main thread.
                    ioScope.launch { maybeReconcileDirectoryGrants() }
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

        /**
         * Construct the singleton off the main thread (Keystore, ONNX model
         * copy, native probe) so startup never blocks the UI thread. Errors
         * propagate as AppManagerInitException for the caller to surface.
         */
        suspend fun warmInstance(context: Context, activity: FragmentActivity? = null): AppManager =
            withContext(Dispatchers.IO) {
                getInstance(context, activity)
            }
    }

}
