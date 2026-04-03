use crate::llm::error::LlmError;

#[test]
fn test_network_error_display() {
    let e = LlmError::NetworkError {
        reason: "timeout".into(),
    };
    let msg = e.display_message();
    assert!(msg.contains("Connection failed"), "got: {}", msg);
    assert!(msg.contains("timeout"), "got: {}", msg);
}

#[test]
fn test_auth_error_display() {
    let e = LlmError::AuthError {
        reason: "bad key".into(),
    };
    let msg = e.display_message();
    assert!(msg.contains("Authentication failed"), "got: {}", msg);
}

#[test]
fn test_model_not_found_display() {
    let e = LlmError::ModelNotFound {
        model_id: "gpt-5".into(),
    };
    let msg = e.display_message();
    assert!(msg.contains("gpt-5"), "got: {}", msg);
}

#[test]
fn test_rate_limited_display() {
    let e = LlmError::RateLimited {
        reason: "wait".into(),
        retry_after_secs: None,
    };
    let msg = e.display_message();
    assert!(msg.contains("Too many requests"), "got: {}", msg);
}

#[test]
fn test_rate_limited_display_with_retry_after() {
    let e = LlmError::RateLimited {
        reason: "slow down".into(),
        retry_after_secs: Some(30),
    };
    let msg = e.display_message();
    assert!(msg.contains("Too many requests"), "got: {}", msg);
    assert!(msg.contains("30s"), "got: {}", msg);
}

#[test]
fn test_api_error_display() {
    let e = LlmError::ApiError {
        status_code: 500,
        reason: "internal".into(),
    };
    let msg = e.display_message();
    assert!(msg.contains("500"), "got: {}", msg);
}
