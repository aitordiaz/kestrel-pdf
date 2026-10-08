# Architecture Specifications & Decision Records (ADRs)

This directory documents the technical architecture, design patterns, subsystem boundaries, and architectural decision records for **Kestrel-PDF**.

---

## Architecture Index

| Number | Title | Status | Date | Summary |
| :--- | :--- | :--- | :--- | :--- |
| **[0001](0001-system-overview.md)** | [System Architecture & Decoupled Engine](0001-system-overview.md) | **Accepted** | 2026-10-06 | Universal egui shell, decoupled core, multi-threaded rendering, and memory virtualization. |
| **[0002](0002-engine-investigation.md)** | [PDF Engines Benchmark & Technical Investigation](0002-engine-investigation.md) | **Accepted** | 2026-10-06 | Comparative analysis of PDFium, MuPDF, Poppler, lopdf, and PDF.js. |
| **[0003](0003-cicd-and-testing-trophy.md)** | [CI/CD Infrastructure & Testing Trophy Strategy](0003-cicd-and-testing-trophy.md) | **Accepted** | 2026-10-06 | Test distribution, GitHub Actions CI matrix, and automated release pipeline. |
| **[0004](0004-synthetic-engine-and-visual-pipeline.md)** | [Synthetic PDF Engine & Viewport Pipeline](0004-synthetic-engine-and-visual-pipeline.md) | **Accepted** | 2026-10-07 | In-memory PDF 1.7 generation, 4-quadrant coordinate rotation, and raster image blitting. |
| **[0005](0005-ui-style-guide-and-design-system.md)** | [Modern UI Style Guide & Professional Design System](0005-ui-style-guide-and-design-system.md) | **Accepted** | 2026-10-08 | Design tokens, warm salmon & light ochre accents, primary CTA affordance, PDF 'as is' focus. |
| **[0006](0006-ux-user-flows-and-navigation.md)** | [UX User Flows, Navigation Model & Document Interaction](0006-ux-user-flows-and-navigation.md) | **Accepted** | 2026-10-08 | In-flow Page Navigator [1]/4, distraction-free canvas, keyboard navigation matrix, primary CTA flow. |
| **[0007](0007-in-place-editing-and-page-tree-architecture.md)** | [In-Place Content Stream Editing, Image Manipulation & Page Tree Mutations](0007-in-place-editing-and-page-tree-architecture.md) | **Accepted** | 2026-10-08 | AST operator surgery, standard Type 1 font embedding, image XObject manipulation, and page tree mutations. |


---

## Architectural Decision Record (ADR) Template

When introducing significant architectural changes, create a new document `XXXX-<title>.md` following this structure:

```markdown
# ADR-XXXX: <Short Title>

- **Status**: [Proposed | Accepted | Superseded | Deprecated]
- **Date**: YYYY-MM-DD
- **Author**: <Agent / Contributor>

## Context & Problem Statement
Describe the architectural dilemma, constraints, performance bottlenecks, or user requirements prompting this decision.

## Considered Options
1. **Option 1**: Description, pros, and cons.
2. **Option 2**: Description, pros, and cons.

## Decision Outcome
Chosen option and why. Detailed technical design of the solution.

## Consequences
- **Positive**: Performance gains, cleaner boundaries, simplified testing.
- **Negative / Trade-offs**: Dependencies, memory overhead, backward compatibility impacts.

## Verification & Testing
How this decision is validated via integration and E2E tests.
```
