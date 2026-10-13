# HANDOFF-0017: Temporary Disabling of Linux and WebAssembly CI/Release Compilation

- **Date**: 2026-10-13
- **Author**: Antigravity & Core Engineering Team
- **Active Version**: v0.3.0
- **Active Branch**: `chore/disable-linux-and-web-ci-compilation` (Merging to `main`)

---

## 1. Executive Summary

To reduce CI pipeline execution time and conserve runner resources during active desktop development for macOS and Windows, compilation and test jobs for Linux (`ubuntu-latest`) and Web (`wasm32-unknown-unknown`) have been temporarily deactivated in GitHub Actions.

---

## 2. Key Accomplishments & Workflow Updates

1. **CI Pipeline (`.github/workflows/ci.yml`)**:
   - `integration-tests`: Removed `ubuntu-latest` from matrix; active runners are `windows-latest` and `macos-latest`.
   - `e2e-smoke-test`: Removed `ubuntu-latest` from matrix; active runners are `windows-latest` and `macos-latest`.
   - `wasm-target-check`: Commented out the WebAssembly build check job until the browser WASM application is ready.
   - `lint-and-format`: Retained on lightweight, fast `ubuntu-latest` runner for `cargo fmt` and `cargo clippy`.

2. **Release Pipeline (`.github/workflows/release.yml`)**:
   - `build-release-artifacts`: Commented out Linux `x86_64-unknown-linux-gnu` target; active release binaries are `windows-x64` and `macos-arm64`.

3. **Governance & Documentation**:
   - Updated `AGENTS.md` to reflect the active matrix requirements.

---

## 3. Re-enabling Instructions

When the Linux desktop packaging or browser WASM applications are ready to be integrated:
1. In `.github/workflows/ci.yml`, add `ubuntu-latest` back to `matrix.os` in `integration-tests` and `e2e-smoke-test`, and uncomment `wasm-target-check`.
2. In `.github/workflows/release.yml`, uncomment the `ubuntu-latest` / `x86_64-unknown-linux-gnu` matrix entry in `build-release-artifacts`.
3. Update `AGENTS.md` matrix checklist.
