package dev.disobey.mango

import dev.disobey.mango.BuildConfig

object FeatureFlags {
    const val AGENTS_ENABLED: Boolean = false

    // Global switch to fully disable LocalLLM runtime behavior without touching feature
    // flags or remote inference flow.
    const val LOCAL_LLM_ENABLED: Boolean = true

    // Managed PPQ account onboarding + provider panel (Waves 2-4).
    // Build-time kill switch; all PPQ UI branches gate on this flag.
    val MANAGED_PPQ_ENABLED: Boolean = BuildConfig.MANAGED_PPQ_ENABLED
}
