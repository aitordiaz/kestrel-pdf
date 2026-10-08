---
name: frontend
description: >-
  Standardized guidelines, visual tokens, UX component patterns, and architectural
  practices for building and extending the Kestrel-PDF frontend using egui and Rust.
---

# Kestrel-PDF Frontend Engineering Skill

This skill guides agents and engineers in developing, styling, and refining the user interface and user experience of **Kestrel-PDF** using `egui` / `eframe`.

---

## 1. Core Visual & UX Principles

1. **High Contrast, Professional Tone**:
   - Primary Accent: Warm Salmon (`#D97757` / `Color32::from_rgb(217, 119, 87)`).
   - Secondary Accent: Light Ochre / Warm Amber (`#E28743` / `Color32::from_rgb(226, 135, 67)`).
   - Base Panels: Neutral Deep Slate (`#0F172A` / `#1E293B`).
   - Subtle Low-Contrast Dividers: Alpha-blended Slate (`Color32::from_rgba_unmultiplied(148, 163, 184, 45)`). Avoid heavy 1px borders around every element.
   - Text: Crisp White (`#F8FAFC`) on dark surfaces; high contrast at all times.
2. **Borderless & Airy Ergonomics ("Breathing Room")**:
   - Secondary and tool buttons are borderless (`Stroke::NONE`) by default with subtle transparent fills and soft hover glows.
   - Never cage buttons inside multiple nested borders ("box inside a box inside a box").
   - Discrete 18px vertical dividers replace harsh full-height separators.
3. **Adaptive Responsive Layout (100% Non-Fullscreen Usability)**:
   - Toolbars and headers must adapt dynamically based on `ui.available_width()`:
     - **Wide (`>= 1050px`)**: Full icon + descriptive label for all controls.
     - **Medium (`780px .. 1050px`)**: Shortened labels and compact caret navigators.
     - **Compact (`< 780px`)**: Iconic mode (`TOOL_*` icons only with rich hover tooltips), compact search, and dynamically truncated document title.
4. **Prominent Primary Actions ("Abrir Fichero")**:
   - The primary call-to-action button (Opening a file) must always be comfortable and immediately discoverable.
   - Solid salmon background fill with white text and clear icon + shortcut hints.
5. **The Document Viewport is Sacred ("PDF As Is")**:
   - The PDF document content must never be altered, tinted, inverted, or cropped by UI overlays.
   - The document canvas must occupy maximum available screen width.
   - Avoid persistent, cluttering sidebars by default. Secondary navigation (outlines, layers) must remain collapsible and on-demand.
6. **In-Flow Page Navigation ("1 / N")**:
   - Direct page navigation lives in the toolbar as a dedicated pill widget:
     `[ < ]  [ current_page ]  /  total_pages  [ > ]`
   - The current page is an interactive text box where users can type a number `X` and press `Enter` to jump.
   - Always clamp page numbers safely to `1..=total_pages`.

---

## 2. Component Design Catalog & Code Patterns

### 2.1 Primary CTA Button Helper

```rust
pub fn render_primary_cta(ui: &mut egui::Ui, icon: &str, label: &str, shortcut: &str) -> egui::Response {
    let text = format!("{} {} {}", icon, label, shortcut).trim().to_string();
    let btn = egui::Button::new(
        egui::RichText::new(text)
            .color(egui::Color32::WHITE)
            .strong()
            .size(13.0),
    )
    .fill(egui::Color32::from_rgb(217, 119, 87)) // #D97757 Salmon
    .rounding(6.0)
    .min_size(egui::vec2(0.0, 30.0));
    ui.add(btn)
}
```

### 2.2 In-Flow Page Navigator Pattern

```rust
pub fn render_page_navigator(
    ui: &mut egui::Ui,
    current_page: &mut usize,
    total_pages: usize,
    input_buffer: &mut String,
) {
    if total_pages == 0 {
        return;
    }
    ui.horizontal(|ui| {
        // Prev button
        let prev_enabled = *current_page > 0;
        if ui.add_enabled(prev_enabled, egui::Button::new("◀").small()).clicked() {
            *current_page = current_page.saturating_sub(1);
            *input_buffer = (*current_page + 1).to_string();
        }

        // Editable page number input
        let text_edit = egui::TextEdit::singleline(input_buffer)
            .desired_width(34.0)
            .font(egui::TextStyle::Monospace)
            .horizontal_align(egui::Align::Center);
        let resp = ui.add(text_edit);
        if resp.lost_focus() || (resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
            if let Ok(parsed) = input_buffer.trim().parse::<usize>() {
                let clamped = parsed.clamp(1, total_pages);
                *current_page = clamped - 1;
                *input_buffer = clamped.to_string();
            } else {
                *input_buffer = (*current_page + 1).to_string();
            }
        }

        // Total denominator
        ui.label(
            egui::RichText::new(format!("/ {}", total_pages))
                .color(egui::Color32::from_rgb(148, 163, 184))
                .size(12.0),
        );

        // Next button
        let next_enabled = *current_page + 1 < total_pages;
        if ui.add_enabled(next_enabled, egui::Button::new("▶").small()).clicked() {
            *current_page = (*current_page + 1).min(total_pages - 1);
            *input_buffer = (*current_page + 1).to_string();
        }
    });
}
```

---

## 3. Keyboard Focus & Event Guards

When developing text inputs (like the page navigator or search bar) in egui:
- Check `ctx.wants_keyboard_input()` before intercepting global document shortcuts (`Cmd+C`, `Ctrl+C`, `Ctrl+A`, Arrow keys, Space).
- Do not let global shortcuts interfere with active user typing.

---

## 4. Testing Frontend Components (Headless Smoke Tests)

All UI components must be testable headlessly in `crates/kestrel-app/tests/smoke_test.rs`:
```rust
let ctx = egui::Context::default();
let raw = egui::RawInput::default();
let out = ctx.run(raw, |ctx| {
    app.render_ui(ctx);
});
// Assert on out.shapes, app state changes, or emitted commands
```
