//! Phase 32 Plan 03: Directory-based RAG actor integration tests.
//!
//! Covers:
//! - Task 1: AppState.directory_sources default + construction of the 6 new AppAction variants.
//! - Task 2: AddDirectorySource / SyncDirectoryFiles / RemoveDirectorySource /
//!   SetDirectoryExclusions / UpdateDirectorySourceBookmark pipeline.

use std::sync::Arc;
use std::time::Duration;

use crate::{
    AppAction, DirectoryFileEntry, EmbeddingStatus, FfiApp, NullBiometricProvider,
    NullEmbeddingProvider, NullKeychainProvider,
};

fn make_app() -> Arc<FfiApp> {
    let app = FfiApp::new(
        "".into(),
        Box::new(NullKeychainProvider),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
        Box::new(NullBiometricProvider),
    );
    // Allow the actor to finish startup (VectorIndex init + load queries).
    std::thread::sleep(Duration::from_millis(100));
    app
}

fn wait() {
    std::thread::sleep(Duration::from_millis(200));
}

// ── Task 1 tests ──────────────────────────────────────────────────────────────

#[test]
fn test_appstate_includes_directory_sources() {
    let app = make_app();
    let state = app.state();
    assert!(
        state.directory_sources.is_empty(),
        "fresh AppState should have empty directory_sources"
    );
}

/// Compile-time check that every new AppAction variant is constructable with the
/// field names native bindings rely on.
#[test]
fn test_appaction_variants_construct() {
    let _add = AppAction::AddDirectorySource {
        display_name: "Vault".into(),
        path: Some("/tmp/x".into()),
        bookmark_data: None,
        tree_uri: None,
        exclusion_globs: vec![],
    };
    let _sync = AppAction::SyncDirectoryFiles {
        source_id: "s".into(),
        files: vec![],
        removed_paths: vec![],
        is_final_batch: true,
    };
    let _remove = AppAction::RemoveDirectorySource {
        source_id: "s".into(),
    };
    let _set_exc = AppAction::SetDirectoryExclusions {
        source_id: "s".into(),
        globs: vec!["*.tmp".into()],
    };
    let _trig = AppAction::TriggerDirectorySync {
        source_id: "s".into(),
    };
    let _bm = AppAction::UpdateDirectorySourceBookmark {
        source_id: "s".into(),
        bookmark_data: vec![1, 2, 3],
    };
    // DirectoryFileEntry constructible from outside the crate root.
    let _entry = DirectoryFileEntry {
        relative_path: "a.md".into(),
        mtime_secs: 100,
        size_bytes: 10,
        content: b"hi".to_vec(),
    };
}

// ── Task 2 tests ──────────────────────────────────────────────────────────────

#[test]
fn test_add_directory_source_inserts_row() {
    let app = make_app();
    app.dispatch(AppAction::AddDirectorySource {
        display_name: "Desktop Notes".into(),
        path: Some("/tmp/notes".into()),
        bookmark_data: None,
        tree_uri: None,
        exclusion_globs: vec![".obsidian/".into()],
    });
    wait();
    let state = app.state();
    assert_eq!(
        state.directory_sources.len(),
        1,
        "AppState should reflect a newly added directory source"
    );
    assert_eq!(state.directory_sources[0].display_name, "Desktop Notes");
    assert_eq!(
        state.directory_sources[0].exclusion_globs,
        vec![".obsidian/".to_string()]
    );
}

#[test]
fn test_sync_directory_files_indexes_changed_files() {
    let app = make_app();
    app.dispatch(AppAction::AddDirectorySource {
        display_name: "Vault".into(),
        path: Some("/tmp/v".into()),
        bookmark_data: None,
        tree_uri: None,
        exclusion_globs: vec![],
    });
    wait();
    let sid = app.state().directory_sources[0].id.clone();

    // Three files added.
    app.dispatch(AppAction::SyncDirectoryFiles {
        source_id: sid.clone(),
        files: vec![
            DirectoryFileEntry {
                relative_path: "a.md".into(),
                mtime_secs: 100,
                size_bytes: 10,
                content: b"alpha body content here for testing".to_vec(),
            },
            DirectoryFileEntry {
                relative_path: "b.md".into(),
                mtime_secs: 200,
                size_bytes: 20,
                content: b"beta body content here for testing".to_vec(),
            },
            DirectoryFileEntry {
                relative_path: "c.md".into(),
                mtime_secs: 300,
                size_bytes: 30,
                content: b"gamma body content here for testing".to_vec(),
            },
        ],
        removed_paths: vec![],
        is_final_batch: true,
    });
    // Give embedding pipeline time — spawn_blocking + VectorIndex save.
    std::thread::sleep(Duration::from_millis(500));

    let state = app.state();
    assert_eq!(
        state.documents.len(),
        3,
        "3 new documents after syncing 3 new files, got {}",
        state.documents.len()
    );

    // Now remove b.md — assert the document disappears.
    app.dispatch(AppAction::SyncDirectoryFiles {
        source_id: sid.clone(),
        files: vec![],
        removed_paths: vec!["b.md".into()],
        is_final_batch: true,
    });
    std::thread::sleep(Duration::from_millis(300));

    let state = app.state();
    assert_eq!(
        state.documents.len(),
        2,
        "1 document removed after removed_paths sync, got {}",
        state.documents.len()
    );
}

#[test]
fn test_sync_directory_files_batching_flushes_vector_index() {
    let app = make_app();
    app.dispatch(AppAction::AddDirectorySource {
        display_name: "Big".into(),
        path: Some("/tmp/big".into()),
        bookmark_data: None,
        tree_uri: None,
        exclusion_globs: vec![],
    });
    wait();
    let sid = app.state().directory_sources[0].id.clone();

    // Batch 1: 50 files, not final.
    let batch1: Vec<DirectoryFileEntry> = (0..50)
        .map(|i| DirectoryFileEntry {
            relative_path: format!("f{i}.md"),
            mtime_secs: 1000 + i as i64,
            size_bytes: 50,
            content: format!("body of file {i} with enough words to chunk").into_bytes(),
        })
        .collect();
    app.dispatch(AppAction::SyncDirectoryFiles {
        source_id: sid.clone(),
        files: batch1,
        removed_paths: vec![],
        is_final_batch: false,
    });
    std::thread::sleep(Duration::from_millis(800));

    // Batch 2: 30 files, final.
    let batch2: Vec<DirectoryFileEntry> = (50..80)
        .map(|i| DirectoryFileEntry {
            relative_path: format!("f{i}.md"),
            mtime_secs: 1000 + i as i64,
            size_bytes: 50,
            content: format!("body of file {i} with enough words to chunk").into_bytes(),
        })
        .collect();
    app.dispatch(AppAction::SyncDirectoryFiles {
        source_id: sid.clone(),
        files: batch2,
        removed_paths: vec![],
        is_final_batch: true,
    });
    std::thread::sleep(Duration::from_millis(800));

    let state = app.state();
    assert_eq!(
        state.documents.len(),
        80,
        "80 documents after 2 batches (50 + 30)"
    );
    assert!(
        state.ingestion_progress.is_none(),
        "ingestion_progress cleared after final batch"
    );
}

#[test]
fn test_remove_directory_source_cascades() {
    let app = make_app();
    app.dispatch(AppAction::AddDirectorySource {
        display_name: "ToRemove".into(),
        path: Some("/tmp/rm".into()),
        bookmark_data: None,
        tree_uri: None,
        exclusion_globs: vec![],
    });
    wait();
    let sid = app.state().directory_sources[0].id.clone();

    // Seed 5 files.
    let files: Vec<DirectoryFileEntry> = (0..5)
        .map(|i| DirectoryFileEntry {
            relative_path: format!("f{i}.md"),
            mtime_secs: 1000 + i as i64,
            size_bytes: 50,
            content: format!("content body of file {i} with some words").into_bytes(),
        })
        .collect();
    app.dispatch(AppAction::SyncDirectoryFiles {
        source_id: sid.clone(),
        files,
        removed_paths: vec![],
        is_final_batch: true,
    });
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(app.state().documents.len(), 5, "precondition: 5 documents");

    app.dispatch(AppAction::RemoveDirectorySource {
        source_id: sid.clone(),
    });
    std::thread::sleep(Duration::from_millis(400));

    let state = app.state();
    assert!(
        state.directory_sources.is_empty(),
        "directory source should be removed"
    );
    assert_eq!(
        state.documents.len(),
        0,
        "all 5 directory-backed documents should be cascade-deleted"
    );
}

#[test]
fn test_set_directory_exclusions_validates() {
    let app = make_app();
    app.dispatch(AppAction::AddDirectorySource {
        display_name: "V".into(),
        path: Some("/tmp/v".into()),
        bookmark_data: None,
        tree_uri: None,
        exclusion_globs: vec![],
    });
    wait();
    let sid = app.state().directory_sources[0].id.clone();

    // Invalid glob — unbalanced bracket.
    app.dispatch(AppAction::SetDirectoryExclusions {
        source_id: sid.clone(),
        globs: vec!["[abc".into()],
    });
    wait();
    let state = app.state();
    // Existing globs should be unchanged (still empty).
    assert!(
        state.directory_sources[0].exclusion_globs.is_empty(),
        "invalid glob must not persist — got {:?}",
        state.directory_sources[0].exclusion_globs
    );
    assert!(
        state.last_error.is_some(),
        "invalid glob must set last_error on AppState"
    );
}

#[test]
fn test_set_directory_exclusions_ok() {
    let app = make_app();
    app.dispatch(AppAction::AddDirectorySource {
        display_name: "V".into(),
        path: Some("/tmp/v".into()),
        bookmark_data: None,
        tree_uri: None,
        exclusion_globs: vec![],
    });
    wait();
    let sid = app.state().directory_sources[0].id.clone();

    app.dispatch(AppAction::SetDirectoryExclusions {
        source_id: sid.clone(),
        globs: vec!["*.tmp".into(), ".obsidian/".into()],
    });
    wait();
    let state = app.state();
    assert_eq!(
        state.directory_sources[0].exclusion_globs,
        vec!["*.tmp".to_string(), ".obsidian/".to_string()]
    );
}

#[test]
fn test_update_bookmark_writes_blob() {
    let app = make_app();
    app.dispatch(AppAction::AddDirectorySource {
        display_name: "iOS".into(),
        path: None,
        bookmark_data: Some(vec![0x00, 0x01]),
        tree_uri: None,
        exclusion_globs: vec![],
    });
    wait();
    let sid = app.state().directory_sources[0].id.clone();

    app.dispatch(AppAction::UpdateDirectorySourceBookmark {
        source_id: sid,
        bookmark_data: vec![0xAA, 0xBB, 0xCC],
    });
    wait();
    // No direct way to inspect bookmark_data in AppState (opaque, per threat model
    // T-32-I2 — never exposed), but the dispatch must not error out. Confirm by
    // verifying that AppState.directory_sources still reports the source.
    let state = app.state();
    assert_eq!(state.directory_sources.len(), 1);
}

// ── Phase 32 Plan 08 tests — get_directory_bookmark accessor ─────────────────

/// Test 1: inserting a source with bookmark_data = Some(blob) and reading it back
/// via get_directory_bookmark returns the exact blob.
#[test]
fn test_get_directory_bookmark_returns_stored_blob() {
    let app = make_app();
    app.dispatch(AppAction::AddDirectorySource {
        display_name: "iOS Vault".into(),
        path: None,
        bookmark_data: Some(vec![0xDE, 0xAD, 0xBE, 0xEF]),
        tree_uri: None,
        exclusion_globs: vec![],
    });
    wait();
    let sid = app.state().directory_sources[0].id.clone();

    let result = app.get_directory_bookmark(sid).expect("should not error");
    assert_eq!(
        result,
        Some(vec![0xDE, 0xAD, 0xBE, 0xEF]),
        "get_directory_bookmark must return the stored blob verbatim"
    );
}

/// Test 2: querying get_directory_bookmark for a source that does not exist
/// returns Ok(None) — not an error.
#[test]
fn test_get_directory_bookmark_missing_source_returns_none() {
    let app = make_app();

    let result = app
        .get_directory_bookmark("nonexistent-id".to_string())
        .expect("should not error for missing source");
    assert_eq!(result, None, "missing source must yield Ok(None)");
}

/// Test 3: a source inserted without bookmark_data (Desktop/Android shape —
/// bookmark_data = None) returns Ok(None), not an error.
#[test]
fn test_get_directory_bookmark_null_blob_returns_none() {
    let app = make_app();
    app.dispatch(AppAction::AddDirectorySource {
        display_name: "Desktop Notes".into(),
        path: Some("/tmp/notes".into()),
        bookmark_data: None,
        tree_uri: None,
        exclusion_globs: vec![],
    });
    wait();
    let sid = app.state().directory_sources[0].id.clone();

    let result = app
        .get_directory_bookmark(sid)
        .expect("should not error for null blob");
    assert_eq!(
        result, None,
        "null bookmark_data column must map to Ok(None), not an error"
    );
}

/// Phase 32 Plan 07: centralised relative-time labels render identically
/// across desktop/iOS/Android. Pure function — no actor, no DB.
#[test]
fn test_relative_time_labels() {
    use crate::relative_time_label;

    // Fixed "now" so the test is deterministic.
    let now: i64 = 1_700_000_000;

    // None → "Never"
    assert_eq!(relative_time_label(None, now), "Never");

    // 30s ago → "Just now"
    assert_eq!(relative_time_label(Some(now - 30), now), "Just now");

    // 0s ago → "Just now"
    assert_eq!(relative_time_label(Some(now), now), "Just now");

    // Clock skew (timestamp in future) → "Just now" (never panic)
    assert_eq!(relative_time_label(Some(now + 120), now), "Just now");

    // 5 minutes ago
    assert_eq!(relative_time_label(Some(now - 300), now), "5m ago");

    // 2 hours ago
    assert_eq!(relative_time_label(Some(now - 7200), now), "2h ago");

    // 1 day ago → "Yesterday" (86400..=172799)
    assert_eq!(relative_time_label(Some(now - 86400), now), "Yesterday");

    // 1.5 days ago → still "Yesterday"
    assert_eq!(
        relative_time_label(Some(now - (86400 + 43200)), now),
        "Yesterday"
    );

    // 3 days ago → "3d ago"
    assert_eq!(relative_time_label(Some(now - 3 * 86400), now), "3d ago");

    // 30 days ago → "30d ago"
    assert_eq!(relative_time_label(Some(now - 30 * 86400), now), "30d ago");
}
