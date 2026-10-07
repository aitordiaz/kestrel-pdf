# Implementation Plans & Spec-Driven Development (SDD)

This directory contains feature specifications, phased engineering roadmaps, and detailed implementation plans for **Kestrel-PDF**.

Under **Spec-Driven Development (SDD)**, every feature, epic, or major fix must have a documented specification before writing production code.

---

## Plans Index

| Number | Title | Target Version | Status | Summary |
| :--- | :--- | :--- | :--- | :--- |
| **[0001](0001-mvp-roadmap.md)** | [MVP Development Roadmap](0001-mvp-roadmap.md) | v0.1.0 – v0.5.0 | **Active** | 5-phase delivery roadmap from high-performance reader to WASM optimization. |
| **[0002](0002-spec-driven-development-framework.md)** | [Spec-Driven Development (SDD) & TDD Framework](0002-spec-driven-development-framework.md) | v0.2.5+ | **Active** | Operational standard for defining goals, contracts, acceptance criteria, and TDD execution. |

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
