//! Integration tests for the pure conversation→markdown formatter in mango_core.
//!
//! These tests exercise `format_conversation_as_markdown_with_now` directly with
//! a fixed timestamp so output is deterministic. The formatter itself does no DB
//! or FFI work; it is the unit under test here.

use mango_core::{format_conversation_as_markdown_with_now, ExportMessage};

fn msg(role: &str, content: &str) -> ExportMessage {
    ExportMessage {
        role: role.to_string(),
        content: content.to_string(),
        image_path: None,
    }
}

fn msg_with_image(role: &str, content: &str, path: &str) -> ExportMessage {
    ExportMessage {
        role: role.to_string(),
        content: content.to_string(),
        image_path: Some(path.to_string()),
    }
}

const FIXED_NOW: &str = "2026-04-21T12:00:00+00:00";

#[test]
fn test_format_empty_conversation() {
    let out = format_conversation_as_markdown_with_now("Chat", &[], FIXED_NOW);
    assert!(out.contains("# Chat"), "missing H1 title: {out}");
    assert!(
        out.contains("_Exported 2026-04-21T12:00:00+00:00_"),
        "missing export metadata: {out}"
    );
    assert!(
        !out.contains("##"),
        "should have no role headings for empty conv: {out}"
    );
    assert!(out.ends_with('\n'), "must end with trailing newline: {out:?}");
}

#[test]
fn test_format_user_assistant_exchange() {
    let msgs = vec![msg("user", "Hi"), msg("assistant", "Hello!")];
    let out = format_conversation_as_markdown_with_now("Chat", &msgs, FIXED_NOW);
    assert!(out.contains("## User\n\nHi"), "missing user block: {out}");
    assert!(
        out.contains("## Assistant\n\nHello!"),
        "missing assistant block: {out}"
    );
    // Order: user must appear before assistant.
    let user_idx = out.find("## User").unwrap();
    let assistant_idx = out.find("## Assistant").unwrap();
    assert!(user_idx < assistant_idx, "order wrong: {out}");
}

#[test]
fn test_format_empty_title_falls_back() {
    let out = format_conversation_as_markdown_with_now("", &[], FIXED_NOW);
    assert!(
        out.contains("# Untitled conversation"),
        "missing fallback title: {out}"
    );
}

#[test]
fn test_format_system_and_image_marker() {
    let msgs = vec![
        msg("system", "You are helpful."),
        msg_with_image("user", "What is this?", "/tmp/x.mgo1"),
    ];
    let out = format_conversation_as_markdown_with_now("Vision", &msgs, FIXED_NOW);
    assert!(out.contains("## System"), "missing system heading: {out}");
    assert!(out.contains("## User"), "missing user heading: {out}");
    assert!(
        out.contains("_[image attachment]_"),
        "missing image marker: {out}"
    );
    // Image marker must come AFTER the user content, not the system message.
    let user_idx = out.find("What is this?").unwrap();
    let img_idx = out.find("_[image attachment]_").unwrap();
    assert!(user_idx < img_idx, "image marker placed before user content: {out}");
}

#[test]
fn test_format_unknown_role_title_cased() {
    let msgs = vec![msg("tool", "ran brave_search")];
    let out = format_conversation_as_markdown_with_now("t", &msgs, FIXED_NOW);
    assert!(out.contains("## Tool"), "expected title-cased Tool heading: {out}");
}
