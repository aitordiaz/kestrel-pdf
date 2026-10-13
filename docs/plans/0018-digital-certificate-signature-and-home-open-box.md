# PLAN-0018: Digital Certificate Signatures with Visual Box Selection & Centered Start Screen Hero Box

- **Target Version**: v0.3.2
- **Author**: Antigravity (AI Pair Programmer)
- **Status**: Completed
- **Date**: 2026-10-13

---

## 1. Executive Summary & Problem Statement

Users requested two essential improvements:
1. **Digital Certificate Signing with Visual Area Selection**:
   - Ability to sign documents using a digital certificate (Certificado Digital, FNMT, DNIe, PKI).
   - Interactive selection of a specific rectangle/square on the PDF document page where the digital signature stamp must be placed.
   - Configuration of signer details (Name, NIF/ID, Issuer CA, Reason, Location) and optional loading of certificate files (`.p12`, `.pfx`, `.crt`, `.cer`).
   - Embedding of the PAdES `/Type /Sig` cryptographic dictionary and visual appearance stream in the target rectangle, as well as high-fidelity visual rendering of the official certificate stamp on the document canvas.
2. **Separated and Centered "Abrir Fichero" Box on Start Screen**:
   - On the start/empty screen (`total_pages == 0`), separate the main "Abrir fichero" box from the header branding.
   - Position the dedicated open file / drop zone card prominently in the center of the viewport with generous breathing room, distinct card frame styling, and clean ergonomics.

---

## 2. Technical Specification

### 2.1 Core Digital Signature Architecture (`kestrel-core`)
- Extend `DigitalSignatureMeta` in `kestrel_core::sign`:
  ```rust
  pub struct DigitalSignatureMeta {
      pub signer_name: String,
      pub signer_id: Option<String>,
      pub issuer: Option<String>,
      pub contact_info: Option<String>,
      pub location: Option<String>,
      pub reason: Option<String>,
      pub signing_time: String,
      pub sha256_fingerprint: String,
      pub target_page: u16,
      pub rect: [f32; 4],
      pub certificate_file: Option<String>,
  }
  ```
- Update `embed_digital_signature`:
  - Use `meta.target_page` to attach the signature widget to the target page.
  - Set the widget `/Rect` to `meta.rect`.
  - Generate an appearance stream (`/AP << /N ... >>`) rendering the official certificate stamp for universal fidelity across external PDF viewers.
  - Compute SHA-256 document digest and embed in `/Contents`.

### 2.2 Interactive Signature Box Selection & Dialog (`kestrel-app`)
- In `CentralPanel`:
  - When in digital signing mode or selecting rect (`digital_signature_selecting_rect`), support mouse click-and-drag on the target page.
  - Render live translucent drag box with dashed ochre stroke and size dimensions `[W × H pt]`.
  - On mouse release, save the rectangle `[min_x, min_y, max_x, max_y]` in PDF points and open the Digital Certificate modal.
  - If a user single-clicks without dragging, center a standard signature rectangle (180 × 60 pt) at the click location.
- Digital Certificate Modal Window:
  - Form fields: Signer Name, NIF/ID, Issuer CA, Reason, Location.
  - Certificate file selector: Button to load `.p12`, `.pfx`, `.crt`, `.cer`.
  - Target area display: Page number and coordinates, with a button to re-select area on the document.
  - Live preview card showing the certificate stamp.
  - Action buttons: "Firmar Documento" (Primary Salmon) and "Cancelar".
- Canvas Stamp Rendering:
  - Render the verified digital certificate stamp on the canvas inside `meta.rect`:
    - Shield/lock icon, "FIRMADO DIGITALMENTE", signer name, NIF, issuer, timestamp, and SHA-256 fingerprint.

### 2.3 Centered and Separated "Abrir Fichero" Hero Box
- In `CentralPanel` when `self.total_pages == 0`:
  - Floating branding above: App logo, "Kestrel-PDF" heading, and description.
  - Dedicated centered hero box:
    - Separated `egui::Frame` with `Theme::PANEL_SURFACE`, subtle border, 14.0 rounding, and generous inner padding.
    - Large open folder icon, prominent primary CTA button ("Abrir fichero PDF"), drag-and-drop hint, and shortcut hint (`Ctrl+O / Cmd+O`).
    - Entire card is interactive and clickable to trigger file selection.
  - Subtle feature badges below with vertical breathing room.

---

## 3. Test-Driven Development (TDD) Strategy

1. **Integration Tests (`crates/kestrel-core/tests/integration_tests.rs`)**:
   - `test_integration_digital_signature_custom_rect_and_page`:
     - Set `DigitalSignatureMeta` with custom `target_page: 1` and `rect: [100.0, 200.0, 320.0, 280.0]`.
     - Embed digital signature into synthetic document.
     - Verify widget annotation exists on Page 1 with exact `Rect` coordinates.
     - Verify signature dictionary `/Type /Sig` exists and SHA-256 fingerprint is verified.
2. **E2E Smoke Tests (`crates/kestrel-app/tests/smoke_test.rs`)**:
   - `test_e2e_digital_certificate_signature_custom_rect_and_modal`:
     - Test opening digital certificate signature modal.
     - Set custom rectangular area `[150.0, 250.0, 350.0, 310.0]` on page 1.
     - Apply digital certificate signature with signer name, NIF, and issuer.
     - Assert `session.digital_signature` contains custom rect and target page.
     - Render UI and assert verified digital stamp shapes are painted on canvas.
   - `test_e2e_home_open_file_box_layout_and_elements`:
     - Verify empty document state renders separated hero box elements ("Abrir fichero PDF", drag hint, logo).
