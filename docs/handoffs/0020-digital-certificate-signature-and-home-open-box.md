# HANDOFF-0020: Digital Certificate Signatures with Visual Area Selection & Centered Start Screen Hero Box

- **Date**: 2026-10-13
- **Author**: Antigravity (AI Pair Programmer)
- **Active Version**: v0.3.2
- **Active Branch**: `main` (Merged PR #32, Tag `v0.3.2` published)

---

## 1. Executive Summary

In response to user requests, version `v0.3.2` introduces:
1. **Digital Certificate Signing with Visual Area Selection ("Firmar con Certificado Digital")**:
   - Extended `DigitalSignatureMeta` in `kestrel-core` to support `signer_id`, `issuer`, `target_page`, `rect: [f32; 4]`, and `certificate_file`.
   - Updated `embed_digital_signature` to attach the signature widget annotation to `meta.target_page` with custom `meta.rect` coordinates and generate an official vector appearance stream (`/AP << /N ... >>`) rendering the verified certificate stamp for universal fidelity across external PDF viewers (Adobe Acrobat, Preview, Edge, Chromium).
   - Added interactive click-and-drag area selection on any PDF page with a crosshair cursor, translucent live preview rectangle (`Color32::from_rgba_unmultiplied(34, 197, 94, 45)`), dashed border, and live dimensions in points (`[W × H pt]`). Quick clicks center a standard 200×80 pt signature box at the click location.
   - Built the "Firmar con Certificado Digital" modal dialog featuring signer metadata fields (Firmante, NIF/ID, Emisor CA, Motivo, Lugar), native certificate file loader button (`rfd::FileDialog`), area summary with re-selection button, and live stamp preview card.
   - Rendered the official digital certificate stamp on the document canvas at `meta.rect` with green security banner, lock icon, signer details, and SHA-256 fingerprint preview.
2. **Centered and Separated "Abrir Fichero" Hero Box on Start Screen**:
   - Separated the header branding (App Logo, "Kestrel-PDF", and subtitle description) so it floats cleanly above without a border.
   - Built a dedicated, prominently centered "Abrir fichero" hero box (`PANEL_SURFACE`, 14.0 rounding, generous padding) with a large open folder icon, prominent salmon primary button ("Abrir fichero PDF"), drag-and-drop prompt, and keyboard shortcut pill (`Ctrl+O / Cmd+O`).
   - Added feature badges below ("⚡ Rendimiento Nativo", "🔒 Firma Digital PAdES", "✏ Edición En-Sitio", "🛡 100% Offline y Seguro") to provide visual breathing room in compliance with `frontend/SKILL.md`.

---

## 2. Test Coverage & Verification Battery

- **Test Suite Results**: 71 automated tests passing (39 E2E smoke tests + 32 integration tests):
  - `test_integration_digital_signature_custom_rect_and_page`: Verifies custom rect, target page, and PAdES `/Type /Sig` dictionary and SHA-256 verification.
  - `test_e2e_digital_certificate_signature_custom_rect_and_modal`: Verifies rectangle selection, modal rendering, signer metadata input, canvas stamp rendering, and serialized PDF verification.
  - `test_e2e_home_open_file_box_separated_layout`: Verifies separated floating branding, centered hero box with CTA and drag hint, and feature badges.
- **Formatting**: `cargo fmt --all -- --check` clean (0 formatting differences).
- **Linter**: `cargo clippy --workspace --all-targets -- -D warnings` clean (0 warnings).
- **Cross-Platform / WASM**: `cargo check -p kestrel-app --target wasm32-unknown-unknown` compiles with 0 errors.

---

## 3. Files Modified & Added

- `crates/kestrel-core/src/sign.rs`: Added `signer_id`, `issuer`, `target_page`, `rect`, and `certificate_file` to `DigitalSignatureMeta`, appearance stream generator `generate_signature_appearance_stream`, and updated `embed_digital_signature`.
- `crates/kestrel-core/src/document.rs`: Updated `save_to_bytes` to pass `meta.target_page` to `sign::embed_digital_signature`.
- `crates/kestrel-core/tests/integration_tests.rs`: Added `test_integration_digital_signature_custom_rect_and_page`.
- `crates/kestrel-app/src/icons.rs`: Added `CERTIFICATE` icon vector glyph.
- `crates/kestrel-app/src/app.rs`: Implemented digital signature selection state, toolbar controls, canvas drag interaction, stamp rendering, modal window, and centered separated start screen layout.
- `crates/kestrel-app/tests/smoke_test.rs`: Added `test_e2e_digital_certificate_signature_custom_rect_and_modal` and `test_e2e_home_open_file_box_separated_layout`.
- `Cargo.toml`: Bumped workspace version to `0.3.2`.
- `docs/plans/0018-digital-certificate-signature-and-home-open-box.md`: Authored and completed SDD plan.
- `docs/plans/README.md`: Updated plans index.
- `docs/handoffs/0020-digital-certificate-signature-and-home-open-box.md`: Recorded session handoff.
- `docs/handoffs/README.md`: Updated handoffs index.
