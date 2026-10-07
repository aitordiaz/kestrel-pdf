# ADR-0003: CI/CD Infrastructure & Testing Trophy Strategy

- **Status**: Accepted
- **Date**: 2026-10-06 (Updated 2026-10-07)
- **Context**: Testing philosophy, multi-platform CI matrix, and automated release pipeline.

---

## 1. Testing Philosophy: The Testing Trophy

In accordance with the **Testing Trophy** model (prioritizing integration tests and end-to-end tests over isolated unit tests), Kestrel-PDF enforces the following testing layers:

```
            /  End-to-End & Smoke Tests  \        <- Full App & Frame Lifecycle (crates/kestrel-app/tests/smoke_test.rs)
           /------------------------------\
          /       Integration Tests        \      <- Document, AST Redactor, LRU Cache, Synthetic Suite (crates/kestrel-core/tests/)
         /----------------------------------\
        /             Unit Tests             \    <- Pure algorithms & Math (aspect ratio, Bézier)
       /--------------------------------------\
      /            Static Analysis             \  <- rustfmt, clippy (-D warnings), cargo check
     /------------------------------------------\
```

### A. Static Analysis (Base)
- **`cargo fmt --all -- --check`**: Enforces unified code formatting across all crates.
- **`cargo clippy --workspace --all-targets -- -D warnings`**: Catches idiomatic bugs, dead code, performance anti-patterns, and memory leaks before compilation.

### B. Unit Tests (Minimal)
- Focused strictly on pure algorithms (Bézier interpolation, coordinate transformations, tile hash collisions).

### C. Integration Tests (Core Focus / Heaviest Layer)
- Located in `crates/kestrel-core/tests/integration_tests.rs`.
- Tests realistic multi-subsystem interactions with reproducible in-memory synthetic PDF documents:
  1. **Document Loading**: In-memory and file-based PDF parsing.
  2. **Concurrent Tile Cache**: Multi-threaded stress testing under load with Rayon/thread workers to guarantee bounded LRU memory eviction.
  3. **True Redaction AST Surgery**: Validates that sensitive strings in content streams are excised and that output documents remain valid.
  4. **Synthetic Visual Showcase**: All 4 orientations (0°, 90°, 180°, 270°), vector tables, and embedded raster images.
  5. **Dynamic Coordinate Mapping**: Verified bottom-left to top-left screen transformation across rotations.
  6. **AcroForms Roundtrip**: Text inputs, checkboxes, and dropdowns serialized and re-parsed.
  7. **Full-Text Search Corpus**: Multi-page case-insensitive keyword and token matching.

### D. End-to-End (E2E) & Smoke Tests (Top)
- Located in `crates/kestrel-app/tests/smoke_test.rs`.
- Validates the complete application lifecycle:
  1. Headless `egui::Context` frame rendering (ensuring 0 panics during update loops).
  2. Dynamic tool switching (Pan $\to$ Select $\to$ FormFill $\to$ Edit $\to$ Sign $\to$ Redact).
  3. Interactive zoom clamping ($10\% - 500\%$) and responsive Fit Width / Fit Page.
  4. Active page orientation and dynamic rotation (`⟲`, `⟳`).
  5. On-canvas visual search highlight rectangles.
  6. Document title and filename display in the application title bar.
  7. Full lifecycle stress session.

---

## 2. GitHub CI/CD Pipeline

The repository uses automated GitHub Actions:

### CI Workflow (`.github/workflows/ci.yml`)
Runs on every Pull Request to `main` and pushes to `main`:
1. **Lint & Formatting**: `cargo fmt` + `cargo clippy`.
2. **Integration Tests Matrix**: Ubuntu, Windows, and macOS.
3. **E2E Smoke Test Matrix**: Headless app verification on Ubuntu, Windows, and macOS.
4. **WebAssembly Target Check**: Compiles with `wasm32-unknown-unknown` to guarantee browser deployment integrity.

### Release Workflow (`.github/workflows/release.yml`)
Triggered automatically when a Git tag matching `v*` (e.g. `v0.2.4`) is pushed:
1. Compiles optimized release binaries:
   - Windows: `x86_64-pc-windows-msvc` (`kestrel-pdf-windows-x64.exe`)
   - macOS: `aarch64-apple-darwin` (`kestrel-pdf-macos-arm64`)
   - Linux: `x86_64-unknown-linux-gnu` (`kestrel-pdf-linux-x64`)
2. Creates an official GitHub Release with binary assets attached and auto-generated release notes.

---

## 3. Branch Protection & PR Governance

Direct pushes to `main` are strictly prohibited. Every roadmap epic or feature requires:
1. Creating a dedicated feature branch (`feat/<name>` or `fix/<name>`).
2. Creating a Pull Request via `gh pr create`.
3. Passing all required status checks across the CI matrix.
4. Merge via Squash PR (`gh pr merge --squash --delete-branch`).
5. Create release tag on `main` to publish binaries.
