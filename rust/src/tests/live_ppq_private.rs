use crate::llm::{BackendConfig, TeeType};

fn ppq_private_backend(api_key: &str) -> BackendConfig {
    BackendConfig {
        id: "ppq-ai".into(),
        name: "PPQ.AI".into(),
        base_url: "https://api.ppq.ai/private/v1/".into(),
        api_key: api_key.into(),
        models: vec!["private/kimi-k2-5".into()],
        tee_type: TeeType::AmdSevSnp,
        max_concurrent_requests: 5,
        supports_tool_use: true,
    }
}

#[tokio::test]
#[ignore]
async fn live_ppq_private_attestation_verifies() {
    let event = crate::llm::ppq_private::verify_backend_attestation(
        &ppq_private_backend("sk-test"),
        &crate::attestation::SnpPolicy::default(),
    )
    .await
    .expect("PPQ private attestation should verify");

    match event {
        crate::attestation::AttestationEvent::Verified { backend_id, .. } => {
            assert_eq!(backend_id, "ppq-ai");
        }
        other => panic!("unexpected attestation event: {other:?}"),
    }
}

#[tokio::test]
#[ignore]
async fn live_ppq_private_rejects_invalid_api_key() {
    use async_openai::types::chat::{
        ChatCompletionRequestMessage, ChatCompletionRequestUserMessageArgs,
    };

    let message = ChatCompletionRequestUserMessageArgs::default()
        .content("Hello from Mango")
        .build()
        .map(ChatCompletionRequestMessage::from)
        .expect("user message should build");

    let error = crate::llm::ppq_private::create_chat_completion(
        &ppq_private_backend("sk-invalid"),
        "private/kimi-k2-5",
        vec![message],
        vec![],
    )
    .await
    .expect_err("invalid PPQ API key should be rejected");

    match error {
        crate::llm::LlmError::AuthError { .. } => {}
        other => panic!("expected AuthError, got {other:?}"),
    }
}
