use std::time::{Duration, Instant};

use crate::llm::backend::{BackendConfig, HealthStatus, TeeType};
use crate::llm::router::{FailoverRouter, HealthState};
use crate::persistence::queries::{
    delete_backend, insert_backend, list_backend_health, list_backends,
    update_backend_display_order, upsert_backend_health, BackendHealthRow, BackendRow,
};
use crate::persistence::Database;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn test_backend(id: &str, models: Vec<&str>) -> BackendConfig {
    BackendConfig {
        id: id.to_string(),
        name: id.to_string(),
        base_url: format!("https://{}.test/v1", id),
        api_key: "test-key".to_string(),
        models: models.into_iter().map(String::from).collect(),
        tee_type: TeeType::IntelTdx,
        max_concurrent_requests: 5,
        supports_tool_use: true,
    }
}

fn in_memory_db() -> Database {
    Database::open(":memory:").unwrap()
}

// ── FailoverRouter: select_backend ───────────────────────────────────────────

#[test]
fn test_select_preferred_healthy() {
    let router = FailoverRouter::new();
    let backends = vec![
        test_backend("a", vec!["llama"]),
        test_backend("b", vec!["llama"]),
    ];
    let result = router.select_backend(&backends, Some("a"), "llama", &[]);
    assert!(result.is_some());
    assert_eq!(result.unwrap().id, "a");
}

#[test]
fn test_select_skips_failed() {
    let mut router = FailoverRouter::new();
    let now = Instant::now();
    router.mark_failed("a", now);
    let backends = vec![
        test_backend("a", vec!["llama"]),
        test_backend("b", vec!["llama"]),
    ];
    let result = router.select_backend(&backends, Some("a"), "llama", &[]);
    assert!(result.is_some());
    assert_eq!(result.unwrap().id, "b");
}

#[test]
fn test_select_skips_missing_model() {
    let router = FailoverRouter::new();
    let backends = vec![
        test_backend("a", vec!["llama"]),
        test_backend("b", vec!["gpt"]),
    ];
    let result = router.select_backend(&backends, Some("a"), "gpt", &[]);
    assert!(result.is_some());
    assert_eq!(result.unwrap().id, "b");
}

#[test]
fn test_select_returns_none_all_excluded() {
    let router = FailoverRouter::new();
    let backends = vec![
        test_backend("a", vec!["llama"]),
        test_backend("b", vec!["llama"]),
    ];
    let result = router.select_backend(&backends, None, "llama", &["a", "b"]);
    assert!(result.is_none());
}

#[test]
fn test_select_walks_by_display_order_after_preferred() {
    let mut router = FailoverRouter::new();
    // "a" is failed; backends slice is ordered by display_order (a=0, b=1, c=2)
    let now = Instant::now();
    router.mark_failed("a", now);
    let backends = vec![
        test_backend("a", vec!["llama"]),
        test_backend("b", vec!["llama"]),
        test_backend("c", vec!["llama"]),
    ];
    let result = router.select_backend(&backends, Some("a"), "llama", &[]);
    assert!(result.is_some());
    // Should return the first healthy backend after preferred (b)
    assert_eq!(result.unwrap().id, "b");
}

// ── FailoverRouter: mark_failed backoff schedule ──────────────────────────────

#[test]
fn test_mark_failed_backoff_schedule() {
    let mut router = FailoverRouter::new();
    let base = Instant::now();

    // First failure: 30s backoff
    router.mark_failed("a", base);
    let until1 = match router.health.get("a").unwrap().state {
        HealthState::Failed { until } => until,
        _ => panic!("expected Failed"),
    };
    let elapsed1 = until1.duration_since(base);
    assert!(
        elapsed1 >= Duration::from_secs(29) && elapsed1 <= Duration::from_secs(31),
        "Expected ~30s, got {:?}",
        elapsed1
    );

    // Second failure: 60s backoff
    router.mark_failed("a", base);
    let until2 = match router.health.get("a").unwrap().state {
        HealthState::Failed { until } => until,
        _ => panic!("expected Failed"),
    };
    let elapsed2 = until2.duration_since(base);
    assert!(
        elapsed2 >= Duration::from_secs(59) && elapsed2 <= Duration::from_secs(61),
        "Expected ~60s, got {:?}",
        elapsed2
    );

    // Third failure: 120s backoff
    router.mark_failed("a", base);
    let elapsed3 = match router.health.get("a").unwrap().state {
        HealthState::Failed { until } => until.duration_since(base),
        _ => panic!("expected Failed"),
    };
    assert!(
        elapsed3 >= Duration::from_secs(119) && elapsed3 <= Duration::from_secs(121),
        "Expected ~120s, got {:?}",
        elapsed3
    );

    // Fourth failure: 240s backoff
    router.mark_failed("a", base);
    let elapsed4 = match router.health.get("a").unwrap().state {
        HealthState::Failed { until } => until.duration_since(base),
        _ => panic!("expected Failed"),
    };
    assert!(
        elapsed4 >= Duration::from_secs(239) && elapsed4 <= Duration::from_secs(241),
        "Expected ~240s, got {:?}",
        elapsed4
    );

    // Fifth failure: 480s (30 * 2^4)
    router.mark_failed("a", base);
    let elapsed5 = match router.health.get("a").unwrap().state {
        HealthState::Failed { until } => until.duration_since(base),
        _ => panic!("expected Failed"),
    };
    assert!(
        elapsed5 >= Duration::from_secs(479) && elapsed5 <= Duration::from_secs(481),
        "Expected ~480s, got {:?}",
        elapsed5
    );

    // Sixth failure: capped at 600s (30 * 2^5 = 960, min(960,600) = 600)
    router.mark_failed("a", base);
    let elapsed6 = match router.health.get("a").unwrap().state {
        HealthState::Failed { until } => until.duration_since(base),
        _ => panic!("expected Failed"),
    };
    assert!(
        elapsed6 >= Duration::from_secs(599) && elapsed6 <= Duration::from_secs(601),
        "Expected ~600s (capped), got {:?}",
        elapsed6
    );
}

// ── FailoverRouter: mark_success ─────────────────────────────────────────────

#[test]
fn test_mark_success_resets() {
    let mut router = FailoverRouter::new();
    let now = Instant::now();
    router.mark_failed("a", now);
    router.mark_success("a");
    assert_eq!(router.health_status("a"), HealthStatus::Healthy);
    assert_eq!(router.health.get("a").unwrap().consecutive_failures, 0);
    assert_eq!(router.health.get("a").unwrap().state, HealthState::Healthy);
}

// ── FailoverRouter: maybe_restore ────────────────────────────────────────────

#[test]
fn test_maybe_restore_expired() {
    let mut router = FailoverRouter::new();
    let now = Instant::now();
    router.mark_failed("a", now);

    // Simulate time passing beyond the 30s backoff
    let future = now + Duration::from_secs(31);
    router.maybe_restore("a", future);

    let state = &router.health.get("a").unwrap().state;
    match state {
        HealthState::Degraded { .. } => {} // correct
        other => panic!("Expected Degraded, got {:?}", other),
    }
}

#[test]
fn test_maybe_restore_not_expired() {
    let mut router = FailoverRouter::new();
    let now = Instant::now();
    router.mark_failed("a", now);

    // Only 5 seconds later, backoff is 30s -- should stay Failed
    let too_soon = now + Duration::from_secs(5);
    router.maybe_restore("a", too_soon);

    let state = &router.health.get("a").unwrap().state;
    match state {
        HealthState::Failed { .. } => {} // correct
        other => panic!("Expected Failed, got {:?}", other),
    }
}

// ── BackendSummary health_status field ───────────────────────────────────────

#[test]
fn test_backend_summary_has_health_status() {
    let backend = test_backend("a", vec!["llama"]);
    let summary = backend.to_summary(true, HealthStatus::Degraded);
    assert_eq!(summary.health_status, HealthStatus::Degraded);
    assert!(summary.is_active);
}

// ── Persistence: insert/delete backend ───────────────────────────────────────

#[test]
fn test_insert_delete_backend() {
    let db = in_memory_db();
    let conn = db.conn();

    // Migration v1 seeds tinfoil, v10 seeds ppq-ai = 2 backends
    let before = list_backends(conn).unwrap();
    assert_eq!(before.len(), 2);

    let new_row = BackendRow {
        id: "test-backend".to_string(),
        name: "Test Backend".to_string(),
        base_url: "https://test.example.com/v1".to_string(),
        model_list: "[\"model-a\"]".to_string(),
        tee_type: "IntelTdx".to_string(),
        display_order: 2,
        is_active: 0,
        created_at: 1700000000,
        max_concurrent_requests: 5,
        supports_tool_use: true,
    };
    insert_backend(conn, &new_row).unwrap();

    let after_insert = list_backends(conn).unwrap();
    assert_eq!(after_insert.len(), 3);
    assert!(after_insert.iter().any(|r| r.id == "test-backend"));

    delete_backend(conn, "test-backend").unwrap();

    let after_delete = list_backends(conn).unwrap();
    assert_eq!(after_delete.len(), 2);
    assert!(!after_delete.iter().any(|r| r.id == "test-backend"));
}

// ── Persistence: update_display_order ────────────────────────────────────────

#[test]
fn test_update_display_order() {
    let db = in_memory_db();
    let conn = db.conn();

    // Insert a backend with display_order=5
    let row = BackendRow {
        id: "ordered-backend".to_string(),
        name: "Ordered Backend".to_string(),
        base_url: "https://ordered.test/v1".to_string(),
        model_list: "[]".to_string(),
        tee_type: "Unknown".to_string(),
        display_order: 5,
        is_active: 0,
        created_at: 1700000001,
        max_concurrent_requests: 5,
        supports_tool_use: true,
    };
    insert_backend(conn, &row).unwrap();

    // Update to display_order=0 (should become first)
    update_backend_display_order(conn, "ordered-backend", -1).unwrap();

    let backends = list_backends(conn).unwrap();
    assert_eq!(backends[0].id, "ordered-backend");
}

// ── Persistence: upsert_backend_health ───────────────────────────────────────

#[test]
fn test_upsert_backend_health() {
    let db = in_memory_db();
    let conn = db.conn();

    let health_row = BackendHealthRow {
        backend_id: "tinfoil".to_string(),
        consecutive_failures: 3,
        last_failure_at: Some(1700000100),
        state: "failed".to_string(),
        backoff_until: Some(1700000400),
    };
    upsert_backend_health(conn, &health_row).unwrap();

    let rows = list_backend_health(conn).unwrap();
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.backend_id, "tinfoil");
    assert_eq!(row.consecutive_failures, 3);
    assert_eq!(row.last_failure_at, Some(1700000100));
    assert_eq!(row.state, "failed");
    assert_eq!(row.backoff_until, Some(1700000400));
}

// ── Persistence: MIGRATION_V4 creates backend_health table ───────────────────

#[test]
fn test_migration_v4_creates_table() {
    let db = in_memory_db();
    let conn = db.conn();

    // If MIGRATION_V4 ran, this query should succeed and return 0
    let count: i64 = conn
        .query_row("SELECT count(*) FROM backend_health", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

// ── mark_failed_429: 429-specific backoff schedule ────────────────────────────

#[test]
fn test_mark_failed_429_backoff_schedule() {
    let mut router = FailoverRouter::new();
    let base = Instant::now();

    // First 429: 1s backoff (1 * 2^0)
    router.mark_failed_429("a", base, None);
    let elapsed1 = match router.health.get("a").unwrap().state {
        HealthState::Failed { until } => until.duration_since(base),
        _ => panic!("expected Failed"),
    };
    assert!(
        elapsed1 >= Duration::from_millis(900) && elapsed1 <= Duration::from_millis(1100),
        "Expected ~1s, got {:?}",
        elapsed1
    );

    // Second 429: 2s backoff (1 * 2^1)
    router.mark_failed_429("a", base, None);
    let elapsed2 = match router.health.get("a").unwrap().state {
        HealthState::Failed { until } => until.duration_since(base),
        _ => panic!("expected Failed"),
    };
    assert!(
        elapsed2 >= Duration::from_millis(1900) && elapsed2 <= Duration::from_millis(2100),
        "Expected ~2s, got {:?}",
        elapsed2
    );

    // Third 429: 4s (1 * 2^2)
    router.mark_failed_429("a", base, None);
    let elapsed3 = match router.health.get("a").unwrap().state {
        HealthState::Failed { until } => until.duration_since(base),
        _ => panic!("expected Failed"),
    };
    assert!(
        elapsed3 >= Duration::from_millis(3900) && elapsed3 <= Duration::from_millis(4100),
        "Expected ~4s, got {:?}",
        elapsed3
    );

    // 4th: 8s, 5th: 16s, 6th: 32s
    router.mark_failed_429("a", base, None);
    router.mark_failed_429("a", base, None);
    router.mark_failed_429("a", base, None);

    // 7th: min(64, 60) = 60s cap
    router.mark_failed_429("a", base, None);
    let elapsed7 = match router.health.get("a").unwrap().state {
        HealthState::Failed { until } => until.duration_since(base),
        _ => panic!("expected Failed"),
    };
    assert!(
        elapsed7 >= Duration::from_secs(59) && elapsed7 <= Duration::from_secs(61),
        "Expected ~60s (capped), got {:?}",
        elapsed7
    );

    // 8th: still 60s (cap holds)
    router.mark_failed_429("a", base, None);
    let elapsed8 = match router.health.get("a").unwrap().state {
        HealthState::Failed { until } => until.duration_since(base),
        _ => panic!("expected Failed"),
    };
    assert!(
        elapsed8 >= Duration::from_secs(59) && elapsed8 <= Duration::from_secs(61),
        "Expected ~60s (still capped), got {:?}",
        elapsed8
    );
}

#[test]
fn test_mark_failed_429_respects_retry_after() {
    let mut router = FailoverRouter::new();
    let base = Instant::now();

    // First 429 with retry_after=45s: computed=1s, max(1, 45)=45s
    router.mark_failed_429("a", base, Some(45));
    let elapsed = match router.health.get("a").unwrap().state {
        HealthState::Failed { until } => until.duration_since(base),
        _ => panic!("expected Failed"),
    };
    assert!(
        elapsed >= Duration::from_secs(44) && elapsed <= Duration::from_secs(46),
        "Expected ~45s from Retry-After, got {:?}",
        elapsed
    );
}

#[test]
fn test_mark_failed_429_independent_of_mark_failed() {
    let mut router = FailoverRouter::new();
    let base = Instant::now();

    // One general failure (consecutive_failures=1, backoff=30s)
    router.mark_failed("a", base);

    // One 429 (consecutive_failures=2, 429 backoff: min(1*2^(2-1), 60)=2s)
    router.mark_failed_429("a", base, None);
    let elapsed = match router.health.get("a").unwrap().state {
        HealthState::Failed { until } => until.duration_since(base),
        _ => panic!("expected Failed"),
    };
    // consecutive_failures is now 2, so backoff = min(1*2^(2-1), 60) = 2s
    assert!(
        elapsed >= Duration::from_millis(1900) && elapsed <= Duration::from_millis(2100),
        "Expected ~2s (consecutive_failures=2 on 429 path), got {:?}",
        elapsed
    );
    // consecutive_failures counter should be 2
    assert_eq!(router.health.get("a").unwrap().consecutive_failures, 2);
}
