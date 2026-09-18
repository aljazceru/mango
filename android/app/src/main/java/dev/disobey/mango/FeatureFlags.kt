package dev.disobey.mango

object FeatureFlags {
    const val AGENTS_ENABLED: Boolean = false

    // Global switch to fully disable LocalLLM runtime behavior without touching feature
    // flags or remote inference flow.
    const val LOCAL_LLM_ENABLED: Boolean = true
}
