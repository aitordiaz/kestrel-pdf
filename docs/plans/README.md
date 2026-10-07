# Implementation Plans & Spec-Driven Development (SDD)

This directory contains feature specifications, phased engineering roadmaps, and detailed implementation plans for **Kestrel-PDF**.

Under **Spec-Driven Development (SDD)**, every feature, epic, or major fix must have a documented specification before writing production code.

---

## Plans Index

| Number | Title | Target Version | Status | Summary |
| :--- | :--- | :--- | :--- | :--- |
| **[0001](0001-mvp-roadmap.md)** | [MVP Development Roadmap](0001-mvp-roadmap.md) | v0.1.0 – v0.5.0 | **Active** | 5-phase delivery roadmap from high-performance reader to WASM optimization. |
| **[0002](0002-spec-driven-development-framework.md)** | [Spec-Driven Development (SDD) & TDD Framework](0002-spec-driven-development-framework.md) | v0.2.5+ | **Active** | Operational standard for defining goals, contracts, acceptance criteria, and TDD execution. |
| **[0003](0003-image-placement-and-form-layout-fidelity.md)** | [Image Placement, Alpha SMask & AcroForm Layout Fidelity](0003-image-placement-and-form-layout-fidelity.md) | v0.2.6 | **Completed** | Fix inverted CTM image placement, SMask alpha, form field bounds, and text occlusion. |
| **[0004](0004-synthetic-stress-matrix-and-layer-fidelity.md)** | [In-Memory Synthetic PDF Stress Matrix, Concurrency & Layer Engine](0004-synthetic-stress-matrix-and-layer-fidelity.md) | v0.2.8 | **Completed** | Exhaustive 8-tier in-memory PDF stress matrix, parallel execution, OCG layers, and zero disk writes. |
| **[0005](0005-responsive-toolbar-and-ux-redefinition.md)** | [Responsive Toolbar Architecture & PDF Reader UX Redefinition](0005-responsive-toolbar-and-ux-redefinition.md) | v0.2.9 | **Completed** | Decouple metadata from actions with 2-tier toolbar, middle-truncation, segmented tools, and zero crowding. |
| **[0006](0006-select-and-copy-text-and-images.md)** | [High-Fidelity Text and Image Selection & Clipboard Copy Engine](0006-select-and-copy-text-and-images.md) | v0.2.10 | **Completed** | Interactive text marquee and click-to-select image, visual highlight bounds, system clipboard copy, and keyboard shortcuts. |
| **[0007](0007-platform-standard-copy-shortcuts.md)** | [Multi-Platform Standard Copy Shortcuts & Keyboard Navigation](0007-platform-standard-copy-shortcuts.md) | v0.2.11 | **Completed** | Standard copy shortcuts per platform (Cmd+C, Ctrl+C, Ctrl+Ins, Key::Copy, Event::Copy), focus guard, Select All. |
| **[0008](0008-tounicode-cmap-and-encoding-fidelity.md)** | [Universal ToUnicode CMap Architecture, Variable-Byte Decoding & Latin-1/WinAnsi Fidelity](0008-tounicode-cmap-and-encoding-fidelity.md) | v0.2.12 | **Completed** | 1-byte vs 2-byte ToUnicode CMaps, codespace range resolution, /Encoding dereferencing, Latin-1 / WinAnsi accented character preservation. |
| **[0009](0009-form-xobject-annot-appearance-and-qr-fidelity.md)** | [Form XObject Hierarchy, Annotation Appearance Streams & 1-Bit PNG Predictor Fidelity](0009-form-xobject-annot-appearance-and-qr-fidelity.md) | v0.2.13 | **Active** | Form XObjects via Do, /Annots /AP /N appearance streams, 1-bit monochrome images, PNG predictor 10..=15, rotated text. |

---

## Implementation Plan Template

Create a new plan file `NNNN-<feature-slug>.md` using this template:

```markdown
# PLAN-NNNN: <Feature Title>

- **Target Version**: vX.Y.Z
- **Author**: <Agent / Contributor>
- **Status**: [Draft | In Progress | Completed]
- **Date**: YYYY-MM-DD

## 1. Objective & Scope
State the user problem, target capability, and boundaries of what is included vs excluded.

## 2. Technical Specification
- **Data Models / Structs**: Key types and state fields.
- **API Contracts / Traits**: Function signatures and error modes.
- **UI / UX Interactions**: Controls, shortcuts, layouts.

## 3. Test-Driven Development (TDD) Strategy
- **Failing Test 1 (Red)**: What integration or E2E test will be written first?
- **Failing Test 2 (Red)**: Edge cases and error scenarios.
- **Synthetic Data Fixtures**: How synthetic documents will be created.

## 4. Phased Execution Steps
1. [ ] Step 1: Write failing test in `tests/`.
2. [ ] Step 2: Implement minimal engine code in `kestrel-core`.
3. [ ] Step 3: Wire into UI in `kestrel-app`.
4. [ ] Step 4: Verify test passes (Green) and run full suite (`cargo test --workspace`).
5. [ ] Step 5: Format and lint (`cargo fmt`, `cargo clippy`).

## 5. Acceptance Criteria
- [ ] Acceptance Criterion 1
- [ ] Acceptance Criterion 2
- [ ] 100% tests passing in CI matrix.
```
