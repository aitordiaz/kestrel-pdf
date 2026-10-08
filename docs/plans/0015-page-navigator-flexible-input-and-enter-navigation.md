# PLAN-0015: Page Navigator Flexible Input Parsing & Enter Jump Engine

- **Target Version**: v0.2.19
- **Author**: Antigravity (AI Pair Programmer)
- **Status**: In Progress
- **Date**: 2026-10-08

---

## 1. Executive Summary & Problem Statement

Users reported that page navigation in the toolbar did not respond when entering fractional or compound page formats:
> *"la navegación de paginas no funciona, si pongo 3/4 y doy al enter, no hace nada, cuando deberia llevarme a la pagina 3"*
> ("Page navigation does not work: if I put 3/4 and press enter, it does nothing, when it should take me to page 3")

### Root Cause Analysis
1. **Strict Integer Parsing Failure**:
   The input text box handler performed `self.page_input_text.trim().parse::<usize>()`. When users naturally typed `"3/4"` or `"3 / 4"` (reflecting the `[ 1 ] / 4` indicator design), the parser failed on the `/` character with `ParseIntError(InvalidDigit)`. On parse error, the code reset `page_input_text` back to `self.current_page.to_string()` without calling `set_current_page`, resulting in a silent failure.
2. **Keyboard Focus & Enter Submissions**:
   When pressing Enter inside a singleline `egui::TextEdit`, the widget surrenders focus on the event frame (`lost_focus() == true`). The check `resp.has_focus() && key_pressed(Key::Enter)` evaluated to `false` when focus was already dropped. Additionally, `Key::NumPadEnter` was not handled.
3. **Viewport Scroll & Visible Page Synchronization**:
   While manual scrolling was possible, the active page counter was not synchronized with the most visible page in the continuous viewport, causing discrepancies between what the user was reading and the toolbar numerator.

---

## 2. Requirements & Specification

### Requirement 1: Flexible Page Input Parser (`parse_page_number`)
- Provide a robust, resilient page parser accepting varied user formats:
  - Fractional strings: `"3/4"`, `"3 / 4"`, `"3/ 4"`, `"3/10"` $\to$ Extracts numerator `3`.
  - Natural language fractions: `"3 of 4"`, `"3 de 4"` $\to$ Extracts numerator `3`.
  - Prefixed strings: `"p3"`, `"p.3"`, `"page 3"`, `"p. 3 / 4"` $\to$ Extracts page `3`.
  - Plain numbers: `"3"`, `" 3 "` $\to$ Extracts `3`.
  - Clamping: Result is clamped to `1..=total_pages` when passed to `set_current_page`.
  - Invalid strings (`"abc"`, `""`): Safely returns `None` and reverts input text without jumping.

### Requirement 2: Reliable Enter & Blur Submissions
- Support both `Key::Enter` and `Key::NumPadEnter`.
- Handle submissions when `(resp.has_focus() && enter_pressed) || resp.lost_focus()`.
- On Enter press while focused:
  - Explicitly surrender widget focus (`resp.surrender_focus()`).
  - Parse input via `parse_page_number`.
  - Call `set_current_page(page)`.
  - Request UI repaint (`ui.ctx().request_repaint()`).
- Expand the input box width from 36px to 54px so inputs like `"3/4"` or `"50/100"` fit comfortably without clipping or scrolling.

### Requirement 3: Viewport Scroll Execution & Visible Page Tracking
- In `CentralPanel`, maintain explicit programmatic scrolling targeting `ui.scroll_to_rect(rect, Some(egui::Align::TOP))` and `response.scroll_to_me(Some(egui::Align::TOP))`.
- Track the page with the highest vertical visibility overlap (`best_visible_page`) in the viewport during manual scrolling.
- Automatically update `current_page` and `page_input_text` to the active visible page when the user is not actively editing the input box (`!page_input_has_focus`) and no programmatic jump is in flight (`!had_scroll_to_page`).

---

## 3. Test & Verification Strategy

1. **Unit Tests**:
   - `test_parse_page_number`: Test matrix with `"3/4"`, `"3 / 4"`, `" 3/4 "`, `"3"`, `" 3 "`, `"3 of 4"`, `"3 de 4"`, `"p3"`, `"p.3"`, `"page 3"`, `"p. 3 / 4"`, `"4/4"`, `""`, `"   "`, `"hello"`.
2. **E2E Smoke Tests**:
   - Test toolbar input simulation with `"3/4"` and Enter key press triggering jump to page 3.
   - Verify `current_page == 3`, `page_input_text == "3"`, and scroll target queued.
   - Test steppers, clamping, and viewport synchronization.
3. **Quality Gates**:
   - `cargo fmt --all -- --check`
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - `cargo test --workspace`
   - `cargo check -p kestrel-app --target wasm32-unknown-unknown`
