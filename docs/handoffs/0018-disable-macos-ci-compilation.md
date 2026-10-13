# HANDOFF-0018: Temporary Disabling of macOS CI/Release Compilation

- **Date**: 2026-10-13
- **Author**: Antigravity & Core Engineering Team
- **Active Version**: v0.3.0
- **Active Branch**: `chore/disable-macos-ci-compilation` (Merging to `main`)

---

## 1. Executive Summary

Following the deactivation of Linux and WebAssembly compilation in HANDOFF-0017, compilation and testing for macOS (`macos-latest`, `aarch64-apple-darwin`) have also been temporarily deactivated in GitHub Actions. This leaves `windows-latest` as the sole active test and release target, dramatically reducing runner minutes and pipeline wait times.

---

## 2. Key Accomplishments & Workflow Updates

1. **CI Pipeline (`.github/workflows/ci.yml`)**:
   - `integration-tests`: Matrix updated to `os: [windows-latest]` only.
   - `e2e-smoke-test`: Matrix updated to `os: [windows-latest]` only.
   - `lint-and-format`: Retained on lightweight `ubuntu-latest` runner for code hygiene.

2. **Release Pipeline (`.github/workflows/release.yml`)**:
   - `build-release-artifacts`: Commented out `macos-latest` / `aarch64-apple-darwin` / `kestrel-pdf-macos-arm64`. Active release binary is `windows-x64.exe`.

3. **Governance & Documentation**:
   - Updated `AGENTS.md` to reflect active checks (`windows-latest` only).

---

## 3. Re-enabling Instructions

When ready to re-enable macOS:
1. In `.github/workflows/ci.yml`, add `macos-latest` back to `matrix.os` in `integration-tests` and `e2e-smoke-test`.
2. In `.github/workflows/release.yml`, uncomment the `macos-latest` / `aarch64-apple-darwin` entry in `build-release-artifacts`.
3. Update `AGENTS.md`.
