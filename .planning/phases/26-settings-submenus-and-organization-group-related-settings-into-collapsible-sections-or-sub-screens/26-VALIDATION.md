---
phase: 26
slug: settings-submenus-and-organization-group-related-settings-into-collapsible-sections-or-sub-screens
status: draft
nyquist_compliant: true
wave_0_complete: true
created: 2026-04-05
---

# Phase 26 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test harness (cargo test) |
| **Config file** | none (inline `#[test]` modules) |
| **Quick run command** | `cargo check -p mango_core && cargo check -p mango-desktop` |
| **Full suite command** | `cargo test -p mango_core 2>&1 | tail -10` |
| **Estimated runtime** | ~26 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo check -p mango_core && cargo check -p mango-desktop`
- **After every plan wave:** Run `cargo test -p mango_core 2>&1 | tail -10`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 26 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 26-01-01 | 01 | 1 | SET-14 | build | `cargo check -p mango_core && grep "settingsProviders" ios/Bindings/mango_core.swift && grep "SettingsProviders" android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt` | ✅ | ⬜ pending |
| 26-02-01 | 02 | 2 | SET-08, SET-09, SET-10, SET-11, SET-12, SET-13 | build + grep | `grep -n "settingsProviders" ios/Mango/Mango/ContentView.swift && grep -n "struct SettingsProvidersView" ios/Mango/Mango/SettingsProvidersView.swift && grep -n "struct SettingsDefaultsView" ios/Mango/Mango/SettingsDefaultsView.swift && grep -n "pushScreen.*settingsProviders" ios/Mango/Mango/SettingsView.swift && cargo check -p mango_core` | ✅ | ⬜ pending |
| 26-02-02 | 02 | 2 | SET-08, SET-09, SET-10, SET-11, SET-12, SET-13 | build + grep | `grep -n "Screen.SettingsProviders" android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt && grep -n "fun SettingsProvidersScreen" android/app/src/main/java/dev/disobey/mango/ui/SettingsProvidersScreen.kt && grep -n "fun SettingsDefaultsScreen" android/app/src/main/java/dev/disobey/mango/ui/SettingsDefaultsScreen.kt && grep -n "Screen.SettingsProviders" android/app/src/main/java/dev/disobey/mango/ui/SettingsScreen.kt && cargo check -p mango_core` | ✅ | ⬜ pending |
| 26-03-01 | 03 | 2 | SET-14 | build + grep | `cargo check -p mango-desktop && grep "pub mod settings_providers" desktop/iced/src/views/mod.rs && grep "pub mod settings_defaults" desktop/iced/src/views/mod.rs && grep "Screen::SettingsProviders" desktop/iced/src/main.rs && grep "Screen::SettingsProviders" desktop/iced/src/views/settings.rs` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Existing infrastructure covers all phase requirements. Phase 26 is a UI reorganization with no new business logic. The Rust core change is adding new Screen enum variants (structural only). No new unit tests are required.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Tapping Providers summary row navigates to Providers sub-screen | SET-09 | UI navigation requires visual verification on device/simulator | On each platform: open Settings, tap Providers row, verify sub-screen appears with provider cards |
| Tapping Defaults summary row navigates to Defaults sub-screen | SET-11 | UI navigation requires visual verification on device/simulator | On each platform: open Settings, tap Defaults row, verify sub-screen appears with model picker + instructions |
| Back navigation from sub-screens returns to Settings | SET-12, SET-13 | Navigation flow requires visual confirmation | On each platform: open sub-screen, tap Back, verify Settings main screen is shown |
| Summary rows show correct context (enabled count, model name) | SET-08, SET-10 | Dynamic data display requires visual confirmation | Enable 2+ providers, verify count shows on Providers row; set default model, verify name shows on Defaults row |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 26s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
