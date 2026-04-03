use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Semaphore;

use super::backend::{BackendConfig, HealthStatus};

/// Per-backend health state used by the router for failover decisions.
#[derive(Clone, Debug, PartialEq)]
pub enum HealthState {
    /// Backend is responding normally.
    Healthy,
    /// Backend recently recovered from failure -- still in observation window.
    Degraded { since: Instant },
    /// Backend is in backoff after consecutive failures. Skipped until `until`.
    Failed { until: Instant },
}

/// Mutable health record for a single backend, maintained by FailoverRouter.
#[derive(Debug)]
pub struct BackendHealth {
    pub consecutive_failures: u32,
    pub last_failure: Option<Instant>,
    pub state: HealthState,
}

impl Default for BackendHealth {
    fn default() -> Self {
        Self {
            consecutive_failures: 0,
            last_failure: None,
            state: HealthState::Healthy,
        }
    }
}

/// Routes requests to backends with exponential-backoff failover.
///
/// Maintains an in-memory health map. The actor loop is responsible for
/// persisting health state to SQLite via `list_backend_health` / `upsert_backend_health`
/// on shutdown and restoring it on startup (Plan 02).
pub struct FailoverRouter {
    pub health: HashMap<String, BackendHealth>,
    /// Per-backend concurrency semaphores. Created or replaced by `init_semaphore`.
    /// Shared with streaming tasks via `Arc` so in-flight permits drain naturally
    /// when the semaphore is replaced on config update.
    semaphores: HashMap<String, Arc<Semaphore>>,
}

impl FailoverRouter {
    /// Create an empty router with no health records (all backends start Healthy).
    pub fn new() -> Self {
        Self {
            health: HashMap::new(),
            semaphores: HashMap::new(),
        }
    }

    /// Initialize (or replace) the concurrency semaphore for a backend.
    ///
    /// Called at startup for each loaded backend and after any config update that
    /// changes `max_concurrent_requests`. Replacing the semaphore means in-flight
    /// permits from the old semaphore are released naturally when those tasks finish.
    pub fn init_semaphore(&mut self, backend_id: &str, max_concurrent: usize) {
        let permits = max_concurrent.max(1);
        self.semaphores
            .insert(backend_id.to_string(), Arc::new(Semaphore::new(permits)));
    }

    /// Return a clone of the semaphore `Arc` for a backend, if one has been initialized.
    ///
    /// The caller passes the `Arc` to `spawn_streaming_task` which then uses
    /// `acquire_owned` inside the async task to respect the concurrency limit.
    pub fn get_semaphore(&self, backend_id: &str) -> Option<Arc<Semaphore>> {
        self.semaphores.get(backend_id).cloned()
    }

    /// Select the best available backend for a request.
    ///
    /// Selection order:
    /// 1. Try `preferred_id` if: healthy/degraded, has `model_id`, not in `exclude_ids`.
    /// 2. Walk `backends` slice in order (sorted by display_order from list_backends).
    ///    Skip: Failed backends whose backoff has not expired, backends missing `model_id`,
    ///    backends in `exclude_ids`.
    /// 3. Return `None` if no backend qualifies.
    pub fn select_backend<'a>(
        &self,
        backends: &'a [BackendConfig],
        preferred_id: Option<&str>,
        model_id: &str,
        exclude_ids: &[&str],
    ) -> Option<&'a BackendConfig> {
        let now = Instant::now();

        // Helper: check if a backend is usable
        let is_usable = |backend: &BackendConfig| -> bool {
            if exclude_ids.contains(&backend.id.as_str()) {
                return false;
            }
            if !backend.models.iter().any(|m| m == model_id) {
                return false;
            }
            // Check health state
            match self.health.get(&backend.id) {
                Some(BackendHealth {
                    state: HealthState::Failed { until },
                    ..
                }) => {
                    now >= *until // only usable if backoff has expired
                }
                _ => true, // Healthy or Degraded -- usable
            }
        };

        // 1. Try preferred backend first
        if let Some(pref_id) = preferred_id {
            if let Some(backend) = backends.iter().find(|b| b.id == pref_id) {
                if is_usable(backend) {
                    return Some(backend);
                }
            }
        }

        // 2. Walk remaining backends in slice order (display_order ascending)
        backends.iter().find(|b| {
            // Don't re-check preferred (already tried)
            if Some(b.id.as_str()) == preferred_id {
                return false;
            }
            is_usable(b)
        })
    }

    /// Record a backend failure, incrementing consecutive_failures and setting
    /// exponential backoff: `min(30 * 2^(failures-1), 600)` seconds.
    ///
    /// Transitions state to `HealthState::Failed { until: now + backoff }`.
    pub fn mark_failed(&mut self, backend_id: &str, now: Instant) {
        let entry = self.health.entry(backend_id.to_string()).or_default();
        entry.consecutive_failures += 1;
        entry.last_failure = Some(now);
        let backoff_secs = std::cmp::min(30u64 * (1u64 << (entry.consecutive_failures - 1)), 600);
        entry.state = HealthState::Failed {
            until: now + Duration::from_secs(backoff_secs),
        };
    }

    /// Record a 429 Rate Limited response, using a shorter backoff curve:
    /// `min(1 * 2^(failures-1), 60)` seconds.
    ///
    /// If `retry_after_secs` is provided (from the HTTP Retry-After header),
    /// the actual backoff is `max(computed_backoff, retry_after_secs)`.
    ///
    /// Per D-03/D-04: reuses the same `consecutive_failures` counter and
    /// `HealthState::Failed { until }` mechanism as `mark_failed`, just with
    /// a shorter 1s base instead of 30s.
    pub fn mark_failed_429(
        &mut self,
        backend_id: &str,
        now: Instant,
        retry_after_secs: Option<u64>,
    ) {
        let entry = self.health.entry(backend_id.to_string()).or_default();
        entry.consecutive_failures += 1;
        entry.last_failure = Some(now);
        let computed = std::cmp::min(1u64 << (entry.consecutive_failures - 1), 60);
        let backoff_secs = if let Some(retry_after) = retry_after_secs {
            std::cmp::max(computed, retry_after)
        } else {
            computed
        };
        entry.state = HealthState::Failed {
            until: now + Duration::from_secs(backoff_secs),
        };
    }

    /// Record a backend success, resetting health to Healthy with 0 failures.
    pub fn mark_success(&mut self, backend_id: &str) {
        let entry = self.health.entry(backend_id.to_string()).or_default();
        entry.consecutive_failures = 0;
        entry.last_failure = None;
        entry.state = HealthState::Healthy;
    }

    /// If the backend is in Failed state and the backoff has expired,
    /// transition to Degraded to allow cautious traffic.
    /// No-op if the backend is Healthy, Degraded, or still in backoff.
    pub fn maybe_restore(&mut self, backend_id: &str, now: Instant) {
        if let Some(entry) = self.health.get_mut(backend_id) {
            if let HealthState::Failed { until } = entry.state {
                if now >= until {
                    entry.state = HealthState::Degraded { since: now };
                }
            }
        }
    }

    /// Map the internal HealthState for a backend to the UniFFI-exported HealthStatus enum.
    pub fn health_status(&self, backend_id: &str) -> HealthStatus {
        match self.health.get(backend_id) {
            None => HealthStatus::Healthy,
            Some(entry) => match entry.state {
                HealthState::Healthy => HealthStatus::Healthy,
                HealthState::Degraded { .. } => HealthStatus::Degraded,
                HealthState::Failed { .. } => HealthStatus::Failed,
            },
        }
    }
}
