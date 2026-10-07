# PLAN-0007: Multi-Platform Standard Copy Shortcuts & Keyboard Navigation

- **Target Version**: v0.2.11
- **Author**: Antigravity (Advanced Agentic Coding)
- **Status**: Completed
- **Date**: 2026-10-07

---

## 1. Objective & Scope

### 1.1 Problem Statement
While Kestrel-PDF v0.2.10 introduced text and image selection and copying, keyboard triggering was limited to a generic `command + Key::C` check. Different desktop environments and operating systems have standard, platform-specific copy conventions:
- **macOS**: `Cmd+C` (`⌘C`), integration clipboard event.
- **Windows & Linux**: `Ctrl+C`, the classic IBM CUA standard `Ctrl+Insert`, dedicated hardware `Copy` key, and integration clipboard events.
- **WebAssembly**: Browser native `Copy` clipboard events and modifier shortcuts.
Additionally, when an input field (such as the live search bar or an interactive form field) has focus, the global shortcut must not steal focus or hijack clipboard copy from that active text edit widget. Finally, users expect platform-standard `Select All` (`Ctrl+A` / `Cmd+A`) to select all page text in `Select` mode.

### 1.2 Target Capabilities
1. **Multi-Platform Copy Shortcut Matrix**:
   - `Event::Copy`: OS / browser native integration copy events.
   - `Key::Copy`: Dedicated hardware multimedia copy key.
   - `Cmd+C`: Primary shortcut on macOS (`modifiers.command` / `modifiers.mac_cmd`).
   - `Ctrl+C`: Primary shortcut on Windows & Linux (`modifiers.command` / `modifiers.ctrl`).
   - `Ctrl+Insert`: Classic IBM CUA secondary copy shortcut standard on Windows & Linux.
2. **Text Input Focus Guard**:
   - Verify `!ctx.wants_keyboard_input()` before processing document-level selection copy, allowing native text boxes (search query, form field inputs) to handle their own copy/cut actions without collision.
3. **Platform-Adaptive UI Tooltips**:
   - Dynamic shortcut badge: displays `⌘C` on macOS and `Ctrl+C / Ctrl+Ins` on Windows/Linux in toolbar and context menu tooltips.
4. **Standard Select All (`Ctrl+A` / `Cmd+A`) in Select Mode**:
   - In `ActiveTool::SelectText`, pressing `Ctrl+A` (Windows/Linux) or `Cmd+A` (macOS) selects all text runs on the active page.

---

## 2. Technical Specification

### 2.1 Shortcut Detection Engine (`crates/kestrel-app/src/app.rs`)

```rust
/// Determines whether standard copy shortcuts were triggered on the active platform.
pub fn is_copy_shortcut_pressed(input: &egui::InputState) -> bool;

/// Determines whether standard select-all shortcuts were triggered on the active platform.
pub fn is_select_all_shortcut_pressed(input: &egui::InputState) -> bool;

/// Returns the platform-appropriate copy shortcut string representation (e.g. "⌘C" or "Ctrl+C / Ctrl+Ins").
pub fn standard_copy_shortcut_str() -> &'static str;

/// Returns the platform-appropriate select-all shortcut string representation (e.g. "⌘A" or "Ctrl+A").
pub fn standard_select_all_shortcut_str() -> &'static str;
```

### 2.2 App Lifecycle Integration

- In `KestrelApp::render_ui`:
  - Check keyboard focus: `let text_edit_focused = ctx.wants_keyboard_input();`
  - If `!text_edit_focused`:
    - On copy shortcut: copy active text or active image.
    - On select-all shortcut: if `active_tool == ActiveTool::SelectText`, select all text on current page.
    - On `Escape`: clear selection.
  - In Tier 2 Toolbar:
    - Display platform-specific shortcut strings in tooltips.

---

## 3. Test-Driven Development (TDD) Strategy

### 3.1 Automated E2E Smoke Tests (`crates/kestrel-app/tests/smoke_test.rs`)
- `test_e2e_platform_copy_shortcuts_matrix`:
  - Test `Cmd+C` (`modifiers.mac_cmd = true`, `key_pressed(Key::C)`).
  - Test `Ctrl+C` (`modifiers.ctrl = true`, `key_pressed(Key::C)`).
  - Test `Ctrl+Insert` (`modifiers.ctrl = true`, `key_pressed(Key::Insert)`).
  - Test `Key::Copy` hardware key.
  - Test `Event::Copy` event.
- `test_e2e_select_all_shortcut`:
  - Test `Ctrl+A` / `Cmd+A` populates all text on the active page into `app.selection`.
- `test_e2e_keyboard_focus_guard`:
  - Test that when text input is focused, document copy is not hijacked.

---

## 4. Phased Execution Steps

1. [x] **Phase 1**: Write Red failing tests in `crates/kestrel-app/tests/smoke_test.rs`.
2. [x] **Phase 2**: Implement shortcut functions and `select_all_current_page()` in `crates/kestrel-app/src/app.rs`.
3. [x] **Phase 3**: Verify tests turn Green.
4. [x] **Phase 4**: Run full verification battery (`cargo fmt`, `cargo clippy`, `cargo test`, `cargo check wasm32`).
5. [x] **Phase 5**: Create PR, verify 8 CI jobs, merge, and tag release `v0.2.11`.

---

## 5. Acceptance Criteria

- [x] `Cmd+C` works on macOS.
- [x] `Ctrl+C` and `Ctrl+Insert` work on Windows & Linux.
- [x] Dedicated `Key::Copy` and `Event::Copy` work across all platforms.
- [x] Tooltips reflect platform conventions (`⌘C` vs `Ctrl+C / Ctrl+Ins`).
- [x] `Ctrl+A` / `Cmd+A` selects all text on current page in Select mode.
- [x] Focused text inputs are not intercepted.
- [x] Zero compiler warnings, 100% CI matrix checks passing.
