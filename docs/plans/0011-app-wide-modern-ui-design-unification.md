# PLAN-0011: App-Wide Modern UI Design System Unification

- **Target Version**: v0.2.15
- **Author**: Antigravity (Advanced Agentic Coding)
- **Status**: Active
- **Date**: 2026-10-08

---

## 1. Executive Summary & Goals

Following the initial design tokens and In-Flow Page Navigator established in `v0.2.14`, this plan unifies the modern UI/UX design system across the **entire Kestrel-PDF application**:
1. **Global egui Visuals (`Theme::apply`)**:
   - Establish a centralized `egui::Visuals` configuration matching the Warm Salmon (`#D97757`), Light Ochre (`#E28743`), Slate 900 (`#0F172A`), Slate 800 (`#1E293B`), and Slate 700 (`#334155`) color system.
   - Every default egui widget (buttons, inputs, comboboxes, checkboxes, scrollbars, separators, menus, modals) automatically adheres to the design palette.
2. **Standardized Reusable Component Helpers (`theme.rs`)**:
   - `primary_button` (Warm Salmon CTA with bold white typography and rounded corners).
   - `secondary_button` (Deep Slate surface with Slate 700 border).
   - `accent_button` (Light Ochre accent).
   - `danger_button` / `warning_button` (Redaction / destructive operations).
   - `pill_frame` and `card_frame` for grouped segmented controls, cards, and modal dialogs.
3. **Application Bars & Panels Overhaul (`app.rs`)**:
   - **Header Bar (Tier 1)**: Slate 900 panel, high-contrast bottom border, prominent Salmon "Abrir fichero" CTA, Ochre "Guardar / Exportar" button, truncated title, and active-state sidebar toggle.
   - **Action Ribbon (Tier 2)**: Slate 800 panel, pill-grouped In-Flow Page Navigator, segmented tool mode selector (Pan, Select, Forms, Edit, Sign, Redact) with solid Salmon active highlights, pill-grouped Zoom steppers, and unified live search bar.
   - **Left Sidebar**: Slate 900 panel, segmented tab bar (Pages, Outlines, Forms, Layers, Search) with accent indicators, and card-framed form fields and search snippets.
   - **Status Bar / Toast**: Deep Slate bottom bar with Ochre info badge and dismiss button.
   - **Signature Modal Window**: Slate 800 frame, Salmon primary CTA, Slate secondary actions, and high-contrast pad canvas.
4. **Distraction-Free PDF Viewport Invariant**:
   - Zero color filtering or distortion applied to the PDF page canvas; neutral Slate 700 backdrop.

---

## 2. Technical Architecture & Component Specifications

### 2.1 Theme & Visuals Configuration (`Theme::apply`)

```rust
pub fn apply(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = Theme::PANEL_DARK;       // #0F172A
    visuals.window_fill = Theme::PANEL_SURFACE;   // #1E293B
    visuals.window_stroke = egui::Stroke::new(1.0, Theme::BORDER_DARK);
    visuals.window_rounding = egui::Rounding::same(10.0);
    visuals.extreme_bg_color = Theme::PANEL_DARK;
    visuals.faint_bg_color = Theme::PANEL_SURFACE;
    visuals.selection.bg_fill = Theme::ACCENT_SALMON;
    visuals.selection.stroke = egui::Stroke::new(1.0, Theme::ACCENT_SALMON_HOVER);
    
    // Inactive, Hovered, and Active widget states
    // ...
    ctx.set_visuals(visuals);
}
```

### 2.2 Component Hierarchy

```
+----------------------------------------------------------------------------------------------------+
| 🦅 Kestrel-PDF  |  [📂 Abrir fichero]  [💾 Guardar]  |  document.pdf (4 pages)  |  [◀ Cerrar panel] | Tier 1 (Header)
+----------------------------------------------------------------------------------------------------+
| [ ◀ ] [ 1 ] / 4 [ ▶ ] | ⟲ ⟳ | [✋ Pan] [📝 Select] [📋 Forms] [✍️ Sign] | Reset Fit [➖] 100% [➕] 🔍 | Tier 2 (Ribbon)
+----------------------------------------------------------------------------------------------------+
| [Sidebar (Tabs)]   |                                                                               |
| - Pages            |                               PDF Page Canvas                                 |
| - Outlines         |                                  (As-Is)                                      |
| - Forms            |                                                                               |
| - Layers           |                                                                               |
| - Search           |                                                                               |
+--------------------+-------------------------------------------------------------------------------+
| ℹ Document saved successfully                                                                   ✖ | Status Bar
+----------------------------------------------------------------------------------------------------+
```

---

## 3. Phased Implementation Plan

- [ ] **Phase 1**: Expand `crates/kestrel-app/src/theme.rs` with `Theme::apply`, widget styling, and reusable button/frame constructors.
- [ ] **Phase 2**: Apply unified design tokens across `app_header`, `action_toolbar`, segmented tool selectors, zoom controls, and search in `crates/kestrel-app/src/app.rs`.
- [ ] **Phase 3**: Upgrade `left_sidebar`, `status_bar`, and `signature_modal_open` window with unified card frames and buttons.
- [ ] **Phase 4**: Update and expand E2E headless smoke tests in `crates/kestrel-app/tests/smoke_test.rs`.
- [ ] **Phase 5**: Run full pre-flight verification battery (`cargo fmt`, `cargo clippy`, `cargo test`, `cargo check wasm32`).
- [ ] **Phase 6**: Open PR to `main`, verify CI matrix, merge, and tag `v0.2.15`.
