package dev.disobey.mango

import android.content.Context
import android.os.Handler
import android.os.Looper
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import dev.disobey.mango.rust.AppAction
import dev.disobey.mango.rust.AppReconciler
import dev.disobey.mango.rust.AppState
import dev.disobey.mango.rust.AppUpdate
import dev.disobey.mango.rust.BusyState
import dev.disobey.mango.rust.EmbeddingProvider
import dev.disobey.mango.rust.EmbeddingStatus
import dev.disobey.mango.rust.FfiApp
import dev.disobey.mango.rust.KeychainProvider
import dev.disobey.mango.rust.OnboardingState
import dev.disobey.mango.rust.AttestationStatus
import dev.disobey.mango.rust.Router
import dev.disobey.mango.rust.Screen
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

class AppManager private constructor(context: Context) : AppReconciler {
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
        AppState(
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
            globalSystemPrompt = null,
            memories = emptyList(),
        ),
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
            override fun store(service: String, key: String, value: String) {
                prefs.edit().putString("$service::$key", value).apply()
            }
            override fun load(service: String, key: String): String? {
                return prefs.getString("$service::$key", null)
            }
            override fun delete(service: String, key: String) {
                prefs.edit().remove("$service::$key").apply()
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
        ffiApp = FfiApp(dataDir, keychain, embedding, embeddingStatus)
        val initial = ffiApp.state()
        state = initial
        lastRevApplied = initial.rev
        _stateFlow = MutableStateFlow(initial)
        ffiApp.listenForUpdates(this)
    }

    fun dispatch(action: AppAction) {
        ffiApp.dispatch(action)
    }

    override fun reconcile(update: AppUpdate) {
        mainHandler.post {
            when (update) {
                is AppUpdate.FullState -> {
                    if (update.v1.rev <= lastRevApplied) return@post
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

        fun getInstance(context: Context): AppManager =
            instance ?: synchronized(this) {
                instance ?: AppManager(context.applicationContext).also { instance = it }
            }
    }
}
