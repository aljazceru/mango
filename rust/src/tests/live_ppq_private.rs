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
        None,
    )
    .await
    .expect_err("invalid PPQ API key should be rejected");

    match error {
        crate::llm::LlmError::AuthError { .. } => {}
        other => panic!("expected AuthError, got {other:?}"),
    }
}

/// Verifies `private/kimi-k3` accepts image input through the real private
/// E2EE transport (the private/* aliases carry no modality metadata in PPQ's
/// /v1/models, so the tier is verified by exercising it).
///
/// Run: PPQ_LIVE_API_KEY=<ppq key> cargo test -p mango_core live_ppq_private_kimi_k3 -- --ignored --nocapture
#[tokio::test]
#[ignore]
async fn live_ppq_private_kimi_k3_accepts_image_input() {
    use async_openai::types::chat::{
        ChatCompletionRequestMessage, ChatCompletionRequestUserMessageContentPart,
        ChatCompletionRequestMessageContentPartImage, ChatCompletionRequestMessageContentPartText,
        ChatCompletionRequestUserMessageArgs, ChatCompletionRequestUserMessageContent, ImageDetail,
        ImageUrl,
    };

    let api_key = std::env::var("PPQ_LIVE_API_KEY").unwrap_or_else(|_| {
        panic!("set PPQ_LIVE_API_KEY to a real PPQ key (throwaway-host key works)")
    });

    // Solid red 64x64 JPEG → data URL (same encode path as prepare_image_for_api).
    use base64::Engine;
    use image::{DynamicImage, Rgb, RgbImage};
    let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(64, 64, Rgb([200, 10, 10])));
    let mut jpeg = Vec::new();
    {
        let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 80);
        enc.encode(
            img.to_rgb8().as_raw(),
            64,
            64,
            image::ExtendedColorType::Rgb8,
        )
        .expect("jpeg encode");
    }
    let data_url = format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&jpeg)
    );

    let message = ChatCompletionRequestUserMessageArgs::default()
        .content(ChatCompletionRequestUserMessageContent::Array(vec![
            ChatCompletionRequestUserMessageContentPart::Text(
                ChatCompletionRequestMessageContentPartText {
                    text: "What is the dominant color of this image? Reply with exactly one word."
                        .to_string(),
                },
            ),
            ChatCompletionRequestUserMessageContentPart::ImageUrl(
                ChatCompletionRequestMessageContentPartImage {
                    image_url: ImageUrl {
                        url: data_url,
                        detail: Some(ImageDetail::Auto),
                    },
                },
            ),
        ]))
        .build()
        .map(ChatCompletionRequestMessage::from)
        .expect("multipart user message should build");

    // Real chat path; also avoids PPQ 400ing on `tools: []`.
    let (core_tx, core_rx) = flume::unbounded::<crate::CoreMsg>();
    let cancel = tokio_util::sync::CancellationToken::new();
    let handle = tokio::spawn(
        crate::llm::ppq_private::run_streaming_chat_completion_from_api_messages(
            ppq_private_backend(&api_key),
            "private/kimi-k3".to_string(),
            vec![message],
            cancel,
            core_tx,
        ),
    );
    handle.await.expect("stream task join");

    let mut answer = String::new();
    let mut errored: Option<crate::llm::LlmError> = None;
    while let Ok(event) = core_rx.try_recv() {
        if let crate::CoreMsg::InternalEvent(inner) = event {
            match *inner {
                crate::llm::streaming::InternalEvent::StreamChunk { token } => {
                    answer.push_str(&token)
                }
                crate::llm::streaming::InternalEvent::StreamError { error } => {
                    errored = Some(error);
                    break;
                }
                crate::llm::streaming::InternalEvent::StreamDone => break,
                _ => {}
            }
        }
    }
    if let Some(error) = errored {
        panic!("private/kimi-k3 image request should succeed: {error}");
    }

    println!("kimi-k3 vision answer: {answer:?}");
    assert!(
        answer.to_lowercase().contains("red"),
        "kimi-k3 must actually read the image (expected 'red', got: {answer:?})"
    );
}

/// Control: same image request against text-only `private/gpt-oss-120b` at
/// realistic size (1536px). Observational — prints whether PPQ rejects it or
/// silently drops the image part.
#[tokio::test]
#[ignore]
async fn live_ppq_private_gpt_oss_120b_image_probe() {
    use async_openai::types::chat::{
        ChatCompletionRequestMessage, ChatCompletionRequestUserMessageContentPart,
        ChatCompletionRequestMessageContentPartImage, ChatCompletionRequestMessageContentPartText,
        ChatCompletionRequestUserMessageArgs, ChatCompletionRequestUserMessageContent, ImageDetail,
        ImageUrl,
    };

    let api_key = std::env::var("PPQ_LIVE_API_KEY").unwrap_or_else(|_| {
        panic!("set PPQ_LIVE_API_KEY to a real PPQ key (throwaway-host key works)")
    });

    use base64::Engine;
    use image::{DynamicImage, Rgb, RgbImage};
    // 1536px like prepare_image_for_api — a tiny probe would mask size effects.
    let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(1536, 1024, Rgb([10, 10, 200])));
    let mut jpeg = Vec::new();
    {
        let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 80);
        enc.encode(
            img.to_rgb8().as_raw(),
            1536,
            1024,
            image::ExtendedColorType::Rgb8,
        )
        .expect("jpeg encode");
    }
    let data_url = format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&jpeg)
    );

    let message = ChatCompletionRequestUserMessageArgs::default()
        .content(ChatCompletionRequestUserMessageContent::Array(vec![
            ChatCompletionRequestUserMessageContentPart::Text(
                ChatCompletionRequestMessageContentPartText {
                    text: "What color is this?".to_string(),
                },
            ),
            ChatCompletionRequestUserMessageContentPart::ImageUrl(
                ChatCompletionRequestMessageContentPartImage {
                    image_url: ImageUrl {
                        url: data_url,
                        detail: Some(ImageDetail::Auto),
                    },
                },
            ),
        ]))
        .build()
        .map(ChatCompletionRequestMessage::from)
        .expect("multipart user message should build");

    let (core_tx, core_rx) = flume::unbounded::<crate::CoreMsg>();
    let cancel = tokio_util::sync::CancellationToken::new();
    let handle = tokio::spawn(
        crate::llm::ppq_private::run_streaming_chat_completion_from_api_messages(
            ppq_private_backend(&api_key),
            "private/gpt-oss-120b".to_string(),
            vec![message],
            cancel,
            core_tx,
        ),
    );
    handle.await.expect("stream task join");

    let mut errored: Option<crate::llm::LlmError> = None;
    let mut answer = String::new();
    while let Ok(event) = core_rx.try_recv() {
        if let crate::CoreMsg::InternalEvent(inner) = event {
            match *inner {
                crate::llm::streaming::InternalEvent::StreamChunk { token } => {
                    answer.push_str(&token)
                }
                crate::llm::streaming::InternalEvent::StreamError { error } => {
                    errored = Some(error);
                    break;
                }
                crate::llm::streaming::InternalEvent::StreamDone => break,
                _ => {}
            }
        }
    }
    match errored {
        Some(error) => println!("gpt-oss-120b image rejection: {error}"),
        None => println!(
            "gpt-oss-120b did NOT reject the image; answered: {answer:?} \
             (gateway likely dropped or ignored the image part)"
        ),
    }
}
