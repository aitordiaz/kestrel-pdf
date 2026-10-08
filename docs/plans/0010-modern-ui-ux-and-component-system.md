# PLAN-0010: Modern UI/UX Redefinition, Design System & In-Flow Page Navigator

- **Target Version**: v0.2.14
- **Author**: Antigravity (Advanced Agentic Coding)
- **Status**: Completed
- **Date**: 2026-10-08

---

## 1. Objective & Scope

### 1.1 Problem Statement
The current Kestrel-PDF interface suffers from visual and UX inefficiencies:
1. **Intrusive Sidebar**: A fixed left panel lists all pages and titles, consuming horizontal screen real estate without providing high navigation value, detracting from the core purpose of a PDF reader (viewing documents).
2. **Missing Page Input Navigation**: Jumping to a specific page requires manual scrolling or sidebar hunting; there is no direct `[ 1 ] / 4` page jump input.
3. **Muted Primary Actions**: Primary actions like "Abrir fichero" (Open file) lack visual prominence and high affordance.
4. **Ad-Hoc Styling**: Inconsistent color tokens and control densities.

### 1.2 Target Deliverables
1. **UI Style Guide & Design System**: Documented in [`docs/architecture/0005-ui-style-guide-and-design-system.md`](../architecture/0005-ui-style-guide-and-design-system.md).
2. **UX User Flows**: Documented in [`docs/architecture/0006-ux-user-flows-and-navigation.md`](../architecture/0006-ux-user-flows-and-navigation.md).
3. **`frontend` Skill**: Reusable agent skill created in `.agents/skills/frontend/SKILL.md` and user configuration.
4. **Component Implementation Rollout**: Phased, iterative delivery starting with:
   - In-Flow Page Navigator (`[ < ] [ X ] / N [ > ]`) with editable text input and clamping.
   - De-emphasis/collapse of the left sidebar pages list.
   - Professional Salmon (`#D97757`) / Light Ochre palette for primary action buttons.
   - Enhanced empty state with prominent "Abrir fichero" CTA.
5. **PR Submission**: Open a dedicated PR against `main` for review without triggering a release tag.

---

## 2. Iterative Component Implementation Strategy

To ensure quality and prevent regressions, improvements are deployed iteratively across 5 distinct component milestones:

### Milestone 1: In-Flow Page Navigator & Sidebar Simplification
- Implement `page_input_text: String` state in `KestrelApp`.
- Render navigation pill in toolbar:
  - Stepper `<` button (decrements page, disabled on page 1).
  - Single-line editable text input displaying `current_page + 1`.
  - Static label `/ total_pages`.
  - Stepper `>` button (increments page, disabled on last page).
  - On Enter, submit, or blur: parse integer, clamp to `1..=total`, scroll to target page.
- Collapse or remove the noisy page list in the left sidebar, freeing horizontal space for full-width PDF reading.

### Milestone 2: Professional Color Tokens & Primary CTA ("Abrir Fichero")
- Define theme palette constants in `kestrel-app`:
  - `ACCENT_SALMON = Color32::from_rgb(217, 119, 87)`
  - `ACCENT_SALMON_HOVER = Color32::from_rgb(229, 139, 109)`
  - `ACCENT_SALMON_ACTIVE = Color32::from_rgb(191, 98, 67)`
  - `PANEL_DARK = Color32::from_rgb(15, 23, 42)`
  - `PANEL_BORDER = Color32::from_rgb(51, 65, 85)`
- Style the primary "Abrir fichero" button with prominent salmon background, crisp white text, generous padding (`Vec2::new(14.0, 7.0)`), and keyboard hint `(Ctrl+O)`.
- Elevate empty state onboarding card with centered primary CTA.

### Milestone 3: Canvas Viewport Maximization ("PDF As Is")
- Ensure the PDF viewport occupies maximum horizontal space when sidebar is closed.
- Keep PDF rendering untouched ("as is" without tint or filter).
- Provide neutral slate surround (`#334155`) for high contrast against white PDF pages.

### Milestone 4: Utility Controls & Responsive Segmented Groups
- Group zoom controls (`[-]`, `100%`, `[+]`, `Fit Width`, `Fit Page`) in a clean pill.
- Group selection tools (`Select Text/Image`, `Pan Hand`, `Search`) in a compact segmented strip.

### Milestone 5: Synthetic Smoke Tests & Verification
- Unit & E2E smoke tests for page navigation text input (direct jump, clamping invalid numbers).
- E2E smoke test verifying prominent Open File button and collapsed sidebar focus.

---

## 3. Phased Execution Checklist

- [x] **Phase 1**: Add UI Style Guide and UX User Flows ADRs to `docs/architecture/`.
- [x] **Phase 2**: Define `frontend` skill in `.agents/skills/frontend/SKILL.md`.
- [x] **Phase 3**: Implement Milestone 1 (In-Flow Page Navigator `[ 1 ] / N` and sidebar simplification).
- [x] **Phase 4**: Implement Milestone 2 (Salmon/Ochre theme tokens and prominent "Abrir fichero" primary CTA).
- [x] **Phase 5**: Update E2E smoke tests in `crates/kestrel-app/tests/smoke_test.rs`.
- [x] **Phase 6**: Pre-flight verification (`cargo fmt`, `cargo clippy`, `cargo test`, `cargo check wasm32`).
- [x] **Phase 7**: Merge Pull Request and publish release `v0.2.14`.
