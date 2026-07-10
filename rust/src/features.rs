//! Build-time product feature gates.

/// Autonomous agent product surface.
///
/// Disabled by default for releases. Build `mango_core` with
/// `--features agents` to expose agent navigation and session actions again.
pub const AGENTS_ENABLED: bool = cfg!(feature = "agents");
