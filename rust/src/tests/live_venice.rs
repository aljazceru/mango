//! Live integration test against api.venice.ai.
//!
//! Run with:
//!   VENICE_API_KEY=<key> cargo test -p mango_core --lib live_venice -- --ignored --nocapture
//!
//! Both tests are `#[ignore]`-gated and skip silently if `VENICE_API_KEY` is
//! unset, so they will never run in CI.

#![allow(unused_imports)]

use crate::llm::{BackendConfig, TeeType};

const VENICE_MODEL: &str = "e2ee-venice-uncensored-24b-p";
const VENICE_BASE_URL: &str = "https://api.venice.ai/api/v1/";

fn skip_if_no_key() -> Option<String> {
    match std::env::var("VENICE_API_KEY") {
        Ok(k) if !k.is_empty() => Some(k),
        _ => {
            eprintln!("VENICE_API_KEY not set; skipping live Venice test");
            None
        }
    }
}

fn venice_backend(api_key: String) -> BackendConfig {
    BackendConfig {
        id: "venice-ai".into(),
        name: "Venice.ai".into(),
        base_url: VENICE_BASE_URL.into(),
        api_key,
        models: vec![VENICE_MODEL.into()],
        tee_type: TeeType::IntelTdx,
        max_concurrent_requests: 5,
        supports_tool_use: false,
    }
}

#[tokio::test]
#[ignore = "live integration test against api.venice.ai; requires VENICE_API_KEY"]
async fn live_venice_attestation_verifies() {
    let Some(api_key) = skip_if_no_key() else {
        return;
    };
    let backend = venice_backend(api_key);
    let policy = crate::attestation::policy::TdxPolicy::default();
    let verified = crate::attestation::venice::ensure_verified_venice_attestation(
        &backend,
        VENICE_MODEL,
        &policy,
    )
    .await
    .expect("Venice attestation must succeed");
    assert_eq!(
        verified.signing_pubkey_uncompressed[0], 0x04,
        "uncompressed secp256k1 pubkey must start with 0x04"
    );
    assert_eq!(verified.submitted_nonce.len(), 32);
    assert!(
        verified.report_blob.len() >= 48,
        "report blob suspiciously small: {} bytes",
        verified.report_blob.len()
    );
    assert_eq!(verified.model, VENICE_MODEL);
}

#[tokio::test]
#[ignore = "live integration test against api.venice.ai; requires VENICE_API_KEY"]
async fn live_venice_chat_completion_e2ee() {
    let Some(api_key) = skip_if_no_key() else {
        return;
    };
    let backend = venice_backend(api_key);

    use async_openai::types::chat::{
        ChatCompletionRequestMessage, ChatCompletionRequestUserMessageArgs,
    };
    let user_msg: ChatCompletionRequestMessage = ChatCompletionRequestUserMessageArgs::default()
        .content("Reply with the single word: VERIFIED")
        .build()
        .expect("user message builds")
        .into();

    let response = crate::llm::venice::create_chat_completion(
        backend,
        VENICE_MODEL.to_string(),
        vec![user_msg],
        None,
    )
    .await
    .expect("Venice E2EE chat completion must round-trip");

    let content = response
        .choices
        .first()
        .and_then(|c| c.message.content.as_deref())
        .unwrap_or("");
    eprintln!("[live-venice] decrypted reply: {content}");
    assert!(
        !content.is_empty(),
        "reply must be non-empty plaintext after E2EE decrypt"
    );
}
