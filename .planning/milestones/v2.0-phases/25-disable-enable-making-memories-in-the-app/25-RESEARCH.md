# Phase 25: Disable/Enable Making Memories — Research

**Researched:** 2026-04-05
**Domain:** Settings persistence, AppState/AppAction UniFFI pattern, multi-platform UI toggle
**Confidence:** HIGH

## Summary

Phase 25 wires a user-facing toggle into the MEMORY settings section (built in Phase 24) that disables or re-enables automatic memory extraction after conversations. The extraction trigger lives in a single location in `rust/src/lib.rs` at the `StreamDone` handler — a one-line guard around `memory::extract::should_extract()` and the subsequent `runtime.spawn()` block. Suppressing extraction requires consulting a new boolean setting (`memories_enabled`) before spawning the background task.

The persistence pattern is fully established: bool settings use the existing `settings` key-value SQLite table via `set_setting(conn, "memories_enabled", "1"/"0")` and `get_setting(conn, "memories_enabled")`, exactly as `brave_api_key_set` was added in Phase 24. AppState gains a `memories_enabled: bool` field, AppAction gains a `SetMemoriesEnabled { enabled: bool }` variant, and UniFFI re-generates bindings for all three platforms.

UI-side, all three Settings screens already have a MEMORY section card. Each platform needs one additional row inside that card: a toggle/switch (iOS `Toggle`, Android Material3 `Switch`, iced `toggler`) labelled "Auto-extract Memories" that reads `appState.memoriesEnabled` and dispatches `SetMemoriesEnabled`. No new screens, no navigation changes.

**Primary recommendation:** Follow the `brave_api_key_set` + `SetBraveApiKey` pattern exactly, substituting a bool for a string. Gate the extraction spawn in `StreamDone` behind `actor_state.app_state.memories_enabled`.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `rusqlite` | 0.38.0 | Settings key-value persistence | Already used for all settings; `set_setting`/`get_setting` functions exist |
| UniFFI | (project) | Generate Swift/Kotlin bindings from Rust types | Required by RMP architecture; all AppState/AppAction changes propagate here |
| iced | 0.14.0 | Desktop UI framework | Already used; `toggler` widget is in iced_widget 0.14.2 |
| SwiftUI | iOS 17+ | iOS UI | `Toggle` is a standard SwiftUI component — no import needed |
| Jetpack Compose + Material3 | Android API 28+ | Android UI | `Switch` is in `androidx.compose.material3` — already imported |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `just` | (project) | Run `just bindings-swift` / `just bindings-kotlin` | After adding any UniFFI-exported type/field |

**No new dependencies required.** This phase is pure wiring of an existing pattern.

## Architecture Patterns

### Pattern 1: Bool Setting in AppState (established — Phase 24)

**What:** Add `pub memories_enabled: bool` to `AppState` (uniffi::Record), add `SetMemoriesEnabled { enabled: bool }` to `AppAction` (uniffi::Enum), persist to the `settings` table as key `"memories_enabled"` with value `"1"` or `"0"`, load at startup.

**Exact precedent:** `brave_api_key_set: bool` + `SetBraveApiKey { api_key: String }` from Phase 24.

```rust
// rust/src/lib.rs — AppState addition
// Phase 25 additions:
/// Whether automatic memory extraction is enabled (per user toggle in Settings).
/// Defaults to true. Stored in settings table as "memories_enabled".
pub memories_enabled: bool,

// AppState::default()
memories_enabled: true,

// AppAction addition
/// Enable or disable automatic memory extraction after each conversation.
SetMemoriesEnabled { enabled: bool },
```

```rust
// Handler (in Action dispatch block, following SetBraveApiKey pattern)
AppAction::SetMemoriesEnabled { enabled } => {
    let _ = persistence::queries::set_setting(
        actor_state.db.conn(),
        "memories_enabled",
        if enabled { "1" } else { "0" },
    );
    actor_state.app_state.memories_enabled = enabled;
}
```

```rust
// Startup load (after brave_api_key_set load, ~line 2495)
let memories_enabled = persistence::queries::get_setting(
    actor_state.db.conn(), "memories_enabled",
).ok().flatten().map(|v| v != "0").unwrap_or(true); // default true = enabled
actor_state.app_state.memories_enabled = memories_enabled;
```

### Pattern 2: Gate Extraction in StreamDone

**What:** Wrap the existing extraction spawn with a guard on `memories_enabled`.

**Location:** `rust/src/lib.rs`, line ~3922, inside `llm::InternalEvent::StreamDone` handler.

```rust
// Phase 25: gate extraction behind user toggle
if actor_state.app_state.memories_enabled
    && memory::extract::should_extract(&messages_snapshot)
{
    // ... existing spawn block unchanged ...
}
```

This is a one-line addition before the existing `if memory::extract::should_extract(...)` check. No other changes to the extraction path.

### Pattern 3: iOS Toggle in Memory Section

**What:** Add a `Toggle` row to `memorySection` in `SettingsView.swift`.

```swift
// ios/Mango/Mango/SettingsView.swift — inside memorySection
Toggle(isOn: Binding(
    get: { appState.memoriesEnabled },
    set: { newValue in appManager.dispatch(.setMemoriesEnabled(enabled: newValue)) }
)) {
    Label("Auto-extract Memories", systemImage: "brain")
}
```

SwiftUI `Toggle` with a `Binding` that dispatches on change is the standard pattern. No local state needed — `appState.memoriesEnabled` is the source of truth.

### Pattern 4: Android Switch in MEMORY Section

**What:** Add a `Switch` row inside the MEMORY card in `SettingsScreen.kt`.

```kotlin
// SettingsScreen.kt — inside the MEMORY card Column/Row
Row(
    modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 12.dp),
    verticalAlignment = Alignment.CenterVertically
) {
    Text(
        "Auto-extract Memories",
        style = MaterialTheme.typography.bodyMedium,
        fontWeight = FontWeight.Medium,
        modifier = Modifier.weight(1f)
    )
    Switch(
        checked = appState.memoriesEnabled,
        onCheckedChange = { checked ->
            onDispatch(AppAction.SetMemoriesEnabled(enabled = checked))
        }
    )
}
```

Material3 `Switch` import: `import androidx.compose.material3.Switch` — not yet in the file, must be added.

### Pattern 5: iced Toggler in Desktop Memory Row

**What:** Extend `views/settings.rs` memory section with an iced `toggler`.

The iced 0.14 `toggler` widget signature:
```rust
// Source: iced_widget 0.14.2
// iced::widget::toggler(is_toggled: bool) -> Toggler<'a, Message, Theme, Renderer>
//   .on_toggle(|new_val| Message) — called when user clicks
//   .label("label text")          — optional label
```

```rust
// desktop/iced/src/views/settings.rs — extend memory section
use iced::widget::toggler;

let memory_toggle = row![
    text("Auto-extract Memories").size(14).color(vc.text),
    iced::widget::Space::new().width(Length::Fill),
    toggler(state.memories_enabled)
        .on_toggle(Message::SettingsMemoriesEnabledToggled)
        .size(20),
]
.align_y(Alignment::Center)
.spacing(8);
```

New `Message` variant in `desktop/iced/src/main.rs`:
```rust
SettingsMemoriesEnabledToggled(bool),
```

Handler:
```rust
Message::SettingsMemoriesEnabledToggled(enabled) => {
    manager.dispatch(AppAction::SetMemoriesEnabled { enabled });
}
```

The `toggler` import must be added to `iced::widget::{}` at line 3 of `settings.rs`.

**Note:** iced `toggler` does NOT need local state in `App::Loaded` because the value comes from `AppState.memories_enabled` (the Rust source of truth). This differs from text inputs that buffer unsubmitted edits.

### Recommended File Change List

| File | Change |
|------|--------|
| `rust/src/lib.rs` | Add `memories_enabled: bool` to AppState, add `SetMemoriesEnabled` to AppAction, add startup load, add handler, add extraction gate |
| `ios/Bindings/mango_core.swift` | Regenerate via `just bindings-swift` |
| `ios/Mango/Mango/SettingsView.swift` | Add Toggle row to `memorySection` |
| `android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt` | Regenerate via `just bindings-kotlin` |
| `android/app/src/main/java/dev/disobey/mango/AppManager.kt` | Add `memoriesEnabled = true` to default AppState constructor |
| `android/app/src/main/java/dev/disobey/mango/ui/SettingsScreen.kt` | Add Switch row to MEMORY card, add `Switch` import |
| `desktop/iced/src/views/settings.rs` | Add `toggler` import, add toggle row in memory section, add parameter to `view()` signature if needed |
| `desktop/iced/src/main.rs` | Add `SettingsMemoriesEnabledToggled(bool)` Message, add handler |
| `rust/src/tests/settings.rs` | Add test for `SetMemoriesEnabled` action persistence |

### Anti-Patterns to Avoid

- **Adding a new migration:** No schema change is needed. The `settings` table already exists (Migration V3) and supports arbitrary key-value pairs. Adding a `memories_enabled` column to the `memories` table or creating a new migration is wrong.
- **Local state for the toggle on desktop:** The toggle reads from `AppState.memories_enabled` directly. Do not add a `settings_memories_enabled: bool` to `App::Loaded` the way text inputs need a local buffer. Iced re-renders from the Rust state on each `FullState` update.
- **Default to false:** The feature has been running since Phase 20. Existing users have memories they've been building. Default must be `true` (enabled) so no regression on upgrade.
- **Re-embedding memories on toggle:** Toggling has no effect on already-stored memories. Only future extraction is affected.
- **Gating retrieval/injection (Phase 21):** The toggle is about *making* new memories, not *using* existing ones. The Phase 21 retrieval/injection path is out of scope.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead |
|---------|-------------|-------------|
| Bool persistence | Custom column / new table | `set_setting(conn, "memories_enabled", "1"/"0")` |
| UniFFI bool type | Custom serialization | `bool` is a primitive UniFFI type, maps directly |
| iOS toggle widget | Custom UISwitch wrapper | SwiftUI `Toggle` |
| Android toggle widget | Custom drawable | Material3 `Switch` |
| Desktop toggle widget | Custom checkbox button | iced `toggler` widget (iced_widget 0.14.2) |

## Common Pitfalls

### Pitfall 1: Forgetting `AppManager.kt` Default State
**What goes wrong:** `AppManager.kt` has a hardcoded `AppState(...)` default for the initial mutable state before the first Rust reconcile arrives. If `memoriesEnabled` is not added there with `memoriesEnabled = true`, the Kotlin data class constructor will fail to compile.
**Why it happens:** The Kotlin `AppState` data class has all fields in its constructor — missing any field is a compile error.
**How to avoid:** Always add new `AppState` fields to the `AppManager.kt` default constructor at line ~80. Precedent: `braveApiKeySet = false` added in Phase 24.

### Pitfall 2: Bindings Not Regenerated
**What goes wrong:** Swift/Kotlin code references `memoriesEnabled` and `setMemoriesEnabled` but the binding files still have the old enum/struct shape. Compile error on iOS/Android.
**Why it happens:** UniFFI bindings must be regenerated whenever AppState or AppAction change.
**How to avoid:** Run `just bindings-swift` and `just bindings-kotlin` after Rust changes. Commit `ios/Bindings/mango_core.swift` and `android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt`.
**Warning signs:** Kotlin/Swift compile error "unresolved identifier memoriesEnabled" or "missing case setMemoriesEnabled".

### Pitfall 3: iced `toggler` Not in Widget Import
**What goes wrong:** `toggler(...)` is not found at compile time.
**Why it happens:** `settings.rs` line 3 only imports `button, column, container, pick_list, row, rule, scrollable, text, text_input`. `toggler` must be added explicitly.
**How to avoid:** Add `toggler` to the `iced::widget::{...}` import list.

### Pitfall 4: Extraction Guard Order
**What goes wrong:** Placing the `memories_enabled` check inside the `if let Some(bid) = bid` block instead of wrapping the outer `if memory::extract::should_extract(...)`.
**Why it happens:** The extraction block is nested with several `if let` guards.
**How to avoid:** The check should be the outermost guard: `if actor_state.app_state.memories_enabled && memory::extract::should_extract(...)`.

### Pitfall 5: `get_setting` returning None for missing key
**What goes wrong:** On existing installs where `memories_enabled` was never written, `get_setting` returns `Ok(None)`. If the unwrap defaults to `false`, existing users will have extraction silently disabled.
**How to avoid:** Default to `true`: `.unwrap_or(true)` — same logic as `attestation_interval_minutes` which defaults to 15 when absent.

## Code Examples

### Reading a bool setting at startup
```rust
// Source: rust/src/lib.rs line ~2492 (brave_api_key_set precedent)
let memories_enabled = persistence::queries::get_setting(
    actor_state.db.conn(), "memories_enabled",
).ok().flatten().map(|v| v != "0").unwrap_or(true);
actor_state.app_state.memories_enabled = memories_enabled;
```

### Full extraction gate in StreamDone
```rust
// Source: rust/src/lib.rs line ~3913 (existing extraction block)
if actor_state.app_state.memories_enabled          // Phase 25 guard
    && memory::extract::should_extract(&messages_snapshot)
{
    let bid = extraction_backend_id
        .as_ref()
        .or_else(|| actor_state.app_state.active_backend_id.as_ref());
    // ... existing spawn block unchanged ...
}
```

### iOS Toggle binding pattern
```swift
// Source: SwiftUI documentation — Toggle with Binding
Toggle(isOn: Binding(
    get: { appState.memoriesEnabled },
    set: { appManager.dispatch(.setMemoriesEnabled(enabled: $0)) }
)) {
    Label("Auto-extract Memories", systemImage: "brain")
}
```

### Test pattern (follows test_brave_api_key_persists)
```rust
// rust/src/tests/settings.rs
#[test]
fn test_memories_enabled_toggle() {
    let app = make_app();

    // Default: memories_enabled = true
    let state = app.state();
    assert!(state.memories_enabled, "memories_enabled should default to true");

    // Disable
    app.dispatch(AppAction::SetMemoriesEnabled { enabled: false });
    wait();
    assert!(!app.state().memories_enabled, "should be false after disable");

    // Re-enable
    app.dispatch(AppAction::SetMemoriesEnabled { enabled: true });
    wait();
    assert!(app.state().memories_enabled, "should be true after re-enable");
}
```

## Environment Availability

Step 2.6: SKIPPED (no external dependencies — purely code changes to existing files).

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in (`cargo test`) |
| Config file | none — standard `cargo test` |
| Quick run command | `cargo test -p mango-core test_memories_enabled -- --nocapture` |
| Full suite command | `cargo test -p mango-core` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SET-MEM-01 | `SetMemoriesEnabled { enabled: false }` persists and updates AppState.memories_enabled | unit | `cargo test -p mango-core test_memories_enabled_toggle` | ❌ Wave 0 |
| SET-MEM-02 | `memories_enabled` defaults to `true` on fresh install | unit | `cargo test -p mango-core test_memories_enabled_toggle` | ❌ Wave 0 |
| SET-MEM-03 | Extraction skipped when `memories_enabled = false` | unit | `cargo test -p mango-core test_memories_disabled_skips_extraction` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p mango-core test_memories_enabled`
- **Per wave merge:** `cargo test -p mango-core`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `rust/src/tests/settings.rs` — add `test_memories_enabled_toggle` covering SET-MEM-01 and SET-MEM-02
- [ ] `rust/src/tests/settings.rs` — add `test_memories_disabled_skips_extraction` covering SET-MEM-03 (may need a helper that calls `should_extract` with extraction disabled)

## Sources

### Primary (HIGH confidence)
- `rust/src/lib.rs` — full AppState struct, AppAction enum, StreamDone handler, startup init block, SetBraveApiKey handler (all verified by direct code read)
- `rust/src/persistence/queries.rs` — `set_setting` / `get_setting` functions (verified)
- `rust/src/persistence/schema.rs` — migration list V1-V15, settings table exists since V3 (verified)
- `ios/Bindings/mango_core.swift` — AppState Swift struct, AppAction enum shape (verified)
- `android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt` — Kotlin AppState, AppAction (verified)
- `android/app/src/main/java/dev/disobey/mango/AppManager.kt` — hardcoded default AppState (verified)
- `desktop/iced/src/views/settings.rs` — memory section UI, `view()` signature (verified)
- `desktop/iced/src/main.rs` — App::Loaded fields, Message enum, handler pattern (verified)
- `ios/Mango/Mango/SettingsView.swift` — `memorySection` structure (verified)
- `android/app/src/main/java/dev/disobey/mango/ui/SettingsScreen.kt` — MEMORY card structure (verified)
- Cargo.lock — iced 0.14.0 / iced_widget 0.14.2 confirmed

### Secondary (MEDIUM confidence)
- iced 0.14 `toggler` widget existence inferred from iced_widget 0.14.2 in Cargo.lock and iced 0.14 public API (toggler has been in iced since 0.4) — confirmed widget name and import path from library history

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new dependencies, all libraries already in project
- Architecture: HIGH — exact precedent in Phase 24 (brave_api_key_set pattern)
- Pitfalls: HIGH — all pitfalls derived from Phase 24 execution experience (documented in STATE.md decisions)

**Research date:** 2026-04-05
**Valid until:** 2026-05-05 (stable codebase, no external API dependencies)
