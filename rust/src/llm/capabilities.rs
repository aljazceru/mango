//! Per-model capability classification.
//!
//! Model metadata is not available from the OpenAI-compatible `/v1/models` endpoint
//! across our supported backends (Tinfoil, PPQ.AI, custom), so we curate capability
//! flags in code using pattern matching on the model id.
//!
//! Bias: **conservative** — when in doubt, return `false`. Returning `false` means
//! the UI hides the image-upload entry point for that model, which the user can
//! work around by switching models. Returning `true` when the model is actually
//! text-only results in a silent failure on send (the model ignores the image
//! or replies "please provide the photo"), which is the exact bug this module
//! exists to prevent.
//!
//! To extend the capability list, add a new case-insensitive substring below.
//! Matching is substring-based rather than exact to accommodate provider-specific
//! prefixes (e.g. `private/qwen3-vl-30b`, `openrouter/google/gemma-3-27b-it`).

/// Returns `true` when the given model id is known to accept multimodal image
/// inputs via the OpenAI-compatible `image_url` content part.
///
/// Input is the raw model id string as it appears in `BackendConfig.models`
/// (e.g. `"llama3-3-70b"`, `"private/qwen3-vl-30b"`, `"gemma3:27b"`).
///
/// Matching is case-insensitive substring. Unknown models return `false`.
#[uniffi::export]
pub fn model_supports_vision(model_id: String) -> bool {
    is_vision_model(&model_id)
}

/// Internal implementation; pure, testable, no UniFFI string clone.
pub fn is_vision_model(model_id: &str) -> bool {
    let id = model_id.to_ascii_lowercase();

    // Gemma 3 / Gemma 4 multimodal (SigLIP vision encoder).
    // Covers: "gemma3:27b", "gemma-3-27b-it", "gemma4-31b", "google/gemma-3-27b".
    if id.contains("gemma3")
        || id.contains("gemma-3")
        || id.contains("gemma4")
        || id.contains("gemma-4")
    {
        return true;
    }

    // Qwen vision-language family.
    // Covers: "qwen-vl", "qwen2-vl", "qwen3-vl-30b", "private/qwen3-vl-30b".
    if id.contains("qwen-vl") || id.contains("qwen2-vl") || id.contains("qwen3-vl") {
        return true;
    }

    // Llama vision-language family. Must NOT match plain "llama3-3-70b".
    // Llama 3.2 ships vision-capable 11B/90B variants with explicit "-vision" suffix.
    // Covers: "llama-3.2-11b-vision", "llama3.2-90b-vision-instruct".
    if id.contains("llama") && id.contains("vision") {
        return true;
    }

    // LLaVA (Large Language and Vision Assistant).
    if id.contains("llava") {
        return true;
    }

    // Pixtral (Mistral vision).
    if id.contains("pixtral") {
        return true;
    }

    // OpenAI multimodal: gpt-4o / gpt-4-turbo / gpt-4.1 / gpt-4.5 / gpt-4v.
    // Must NOT match gpt-3.5 or gpt-oss-120b (text-only Tinfoil model).
    if id.contains("gpt-4o")
        || id.contains("gpt-4-turbo")
        || id.contains("gpt-4.1")
        || id.contains("gpt-4.5")
        || id.contains("gpt-4v")
    {
        return true;
    }

    // Anthropic Claude 3 and 4 families are all multimodal.
    // Covers: "claude-3-haiku", "claude-3-5-sonnet", "claude-4-opus".
    if id.contains("claude-3") || id.contains("claude-4") {
        return true;
    }

    // Google Gemini 1.5+ and 2.x are multimodal.
    // Covers: "gemini-1.5-pro", "gemini-2.0-flash", "gemini-pro-vision".
    if id.contains("gemini-1.5") || id.contains("gemini-2") || id.contains("gemini-pro-vision") {
        return true;
    }

    // Conservative default.
    false
}

#[cfg(test)]
mod tests {
    use super::is_vision_model;

    #[test]
    fn vision_models_return_true() {
        // Tinfoil Gemma 3 (user-visible label variants seen in the wild).
        assert!(is_vision_model("gemma3:27b"), "gemma3:27b should be vision");
        assert!(
            is_vision_model("gemma-3-27b-it"),
            "gemma-3-27b-it should be vision"
        );
        // User-reported label from the original debug session.
        assert!(is_vision_model("gemma4-31b"), "gemma4-31b should be vision");
        assert!(
            is_vision_model("google/gemma-3-27b"),
            "prefixed gemma-3 should be vision"
        );

        // PPQ.AI private vision model.
        assert!(
            is_vision_model("private/qwen3-vl-30b"),
            "private/qwen3-vl-30b should be vision"
        );
        assert!(is_vision_model("qwen2-vl-7b"), "qwen2-vl should be vision");

        // Llama vision variants (NOT plain llama3-3-70b).
        assert!(
            is_vision_model("llama-3.2-11b-vision-instruct"),
            "llama-3.2-vision should be vision"
        );

        // Other vision families.
        assert!(is_vision_model("llava-1.6"), "llava should be vision");
        assert!(is_vision_model("pixtral-12b"), "pixtral should be vision");
        assert!(is_vision_model("gpt-4o"), "gpt-4o should be vision");
        assert!(
            is_vision_model("gpt-4o-mini"),
            "gpt-4o-mini should be vision"
        );
        assert!(
            is_vision_model("gpt-4-turbo"),
            "gpt-4-turbo should be vision"
        );
        assert!(
            is_vision_model("claude-3-5-sonnet"),
            "claude-3-5-sonnet should be vision"
        );
        assert!(
            is_vision_model("claude-4-opus"),
            "claude-4-opus should be vision"
        );
        assert!(
            is_vision_model("gemini-1.5-pro"),
            "gemini-1.5-pro should be vision"
        );
    }

    #[test]
    fn text_only_models_return_false() {
        // Seeded Tinfoil models (all text-only in current deployment).
        assert!(
            !is_vision_model("llama3-3-70b"),
            "llama3-3-70b is text-only"
        );
        assert!(
            !is_vision_model("deepseek-r1-0528"),
            "deepseek-r1-0528 is text-only"
        );
        assert!(!is_vision_model("kimi-k2-5"), "kimi-k2-5 is text-only");

        // Seeded PPQ.AI private models that are text-only.
        assert!(
            !is_vision_model("private/kimi-k2-5"),
            "private/kimi-k2-5 is text-only"
        );
        assert!(
            !is_vision_model("private/deepseek-r1-0528"),
            "private/deepseek-r1-0528 is text-only"
        );
        assert!(
            !is_vision_model("private/gpt-oss-120b"),
            "private/gpt-oss-120b is text-only (must not match gpt-4 patterns)"
        );
        assert!(
            !is_vision_model("private/llama3-3-70b"),
            "private/llama3-3-70b is text-only (must not match llama+vision combo)"
        );

        // OpenAI text-only variants.
        assert!(
            !is_vision_model("gpt-3.5-turbo"),
            "gpt-3.5-turbo is text-only"
        );
        assert!(
            !is_vision_model("gpt-oss-120b"),
            "gpt-oss-120b must not match gpt-4 patterns"
        );

        // Gemini 1.0 (text-only legacy).
        assert!(
            !is_vision_model("gemini-1.0-pro"),
            "gemini-1.0-pro is text-only"
        );

        // Claude 2 (pre-vision).
        assert!(!is_vision_model("claude-2.1"), "claude-2.1 is text-only");

        // Empty / unknown.
        assert!(!is_vision_model(""), "empty string returns false");
        assert!(
            !is_vision_model("random-local-model"),
            "unknown returns false"
        );
    }

    #[test]
    fn matching_is_case_insensitive() {
        assert!(is_vision_model("GEMMA3:27B"));
        assert!(is_vision_model("Gpt-4o"));
        assert!(is_vision_model("Claude-3-Haiku"));
    }
}
