# PLAN-0019: System Certificate Auto-Detection & Automated Metadata Extraction

- **Target Version**: v0.3.3
- **Author**: Antigravity (AI Pair Programmer)
- **Status**: Draft
- **Date**: 2026-10-19

---

## 1. Objective & Scope

### 1.1 Problem Statement
In `v0.3.2`, users can initiate digital signing with visual area selection and manually input signer metadata (Firmante, NIF/ID, Emisor CA, Motivo, Lugar) or point to a certificate file. However, users must manually type all metadata or manually select a file every time. In enterprise and Spanish administration workflows (e.g. FNMT, DNIe, ACCV, Camerfirma, Izenpe, corporate PKI), personal certificates are already installed directly into the operating system's personal certificate store (e.g. Windows "MY" personal store, Linux/macOS standard keystores).

### 1.2 User Requirements
1. **Auto-Detect Installed System Certificates**: Automatically detect all personal digital certificates installed on the user's operating system without manual user searching.
2. **Interactive Certificate Selector**: Display a clean, responsive selector (dropdown/list) in the "Firmar con Certificado Digital" modal allowing the user to view and choose among detected certificates.
3. **Automated Metadata Extraction & Autofill**: Upon choosing an installed certificate (or loading one from disk), automatically extract and populate all signer details:
   - **Firmante (Signer Name)**: Extracted from certificate Subject Common Name (`CN`) or GivenName + Surname.
   - **NIF / NIE / CIF**: Extracted from Subject `serialNumber` (OID `2.5.4.5`) or embedded identifier in `CN`.
   - **Emisor / CA**: Extracted from Issuer Common Name (`CN`) or Organization (`O`).
   - **Validez & Estado**: Formatted validity window (`not_before` to `not_after`) and active/expired status indicator.
   - **Huella SHA-256**: Hex fingerprint of certificate DER.
   - **Motivo**: Pre-filled with standard signing purpose ("Documento firmado digitalmente").
4. **File Picker Fallback with Extraction**: Retain manual file picker for `.p12`, `.pfx`, `.crt`, `.cer`, `.pem`, which also extracts and autofills fields immediately.
5. **Cross-Platform & WASM Safety**: Native detection on Windows (CryptoAPI "MY" store) and Unix systems, with clean fallback and zero panics on WebAssembly (`wasm32-unknown-unknown`).

---

## 2. Technical Specification

### 2.1 Data Models & Structs (`kestrel-core::cert`)

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CertificateSource {
    WindowsStore(String),     // e.g. "MY" (Personal Store)
    SystemStore(String),      // System path or keystore
    UserDirectory(String),    // ~/.config/kestrel/certificates or user cert paths
    File(String),             // Loaded from disk
    SyntheticMock,            // In-memory testing fixture
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledCertificate {
    pub id: String,                    // Unique ID / thumbprint / hash
    pub subject_name: String,          // CN or formatted full name
    pub subject_id: Option<String>,    // Extracted NIF / NIE / CIF (OID 2.5.4.5 or parsed from CN)
    pub issuer: String,                // Issuer CN or Org (e.g. "AC FNMT Usuarios", "DNIe")
    pub not_before: String,            // Formatted date string (YYYY-MM-DD)
    pub not_after: String,             // Formatted date string (YYYY-MM-DD)
    pub is_valid: bool,                // Whether current system time is within validity window
    pub serial_number: String,         // Hex serial number
    pub sha256_fingerprint: String,    // Hex SHA-256 fingerprint
    pub raw_der: Vec<u8>,              // Raw DER bytes for PAdES embedding
    pub source: CertificateSource,     // Origin of the certificate
}
```

### 2.2 Core API Contracts (`kestrel-core::cert`)

- `pub fn parse_x509_der(der_bytes: &[u8], source: CertificateSource) -> Result<InstalledCertificate>`
- `pub fn parse_x509_pem(pem_str: &str, source: CertificateSource) -> Result<Vec<InstalledCertificate>>`
- `pub fn parse_certificate_bytes(bytes: &[u8], source: CertificateSource) -> Result<Vec<InstalledCertificate>>`
- `pub fn detect_system_certificates() -> Vec<InstalledCertificate>`
  - Windows: enumerates `CertOpenSystemStoreW(0, "MY")` using `windows-sys` and parses all personal certificates.
  - Linux / macOS: scans user certificate directories (`~/.config/kestrel/certificates`, `~/.local/share/ca-certificates`, `/etc/ssl/certs`).
  - WASM: returns empty list safely.
- `pub fn extract_spanish_nif_or_id(raw_serial: Option<&str>, common_name: &str) -> Option<String>`

### 2.3 UI / UX Flow in `kestrel-app`

1. **Modal Opening**:
   - When entering the Digital Signature Modal, `detect_system_certificates()` is called.
   - If certificates are found, the first valid certificate is auto-selected and its metadata autofills the inputs.
2. **Certificate Selection Dropdown**:
   - Displays a clean egui ComboBox with icon, Signer Name, NIF, Issuer, and validity pill.
   - Changing selection updates all input fields instantaneously.
3. **Active Certificate Badge**:
   - Shows validity status (green if valid, amber/red if expired), serial number, and source.
4. **File Picker**:
   - Selecting a file parses it and immediately autofills the inputs.
5. **Editing**:
   - All fields remain editable for custom adjustments.

---

## 3. Test-Driven Development (TDD) Strategy

- **Test 1 (Integration)**: `test_integration_x509_metadata_extraction_and_nif_parsing` in `kestrel-core`.
  - Tests parsing of synthetic DER and PEM certificates with Spanish NIF in OID `2.5.4.5` and in CN format.
  - Verifies extraction of `subject_name`, `subject_id`, `issuer`, validity dates, and SHA-256 fingerprint.
- **Test 2 (Integration)**: `test_integration_parse_certificate_bytes_der_and_pem`.
  - Verifies multi-format parsing (DER bytes vs PEM strings).
- **Test 3 (E2E Smoke)**: `test_e2e_auto_detect_certificates_selection_and_autofill` in `kestrel-app`.
  - Verifies modal rendering with detected certificates, dropdown selection change, autofill of text inputs, and stamping on document.
- **Test 4 (E2E Smoke)**: `test_e2e_certificate_file_load_and_autofill`.
  - Verifies loading certificate bytes updates `signer_name_input`, `signer_id_input`, and `signer_issuer_input`.

---

## 4. Phased Execution Steps

1. [x] **Phase 1**: Add `x509-parser` dependency and author `kestrel-core/src/cert.rs`.
2. [x] **Phase 2**: Implement X.509 DER/PEM parser, Spanish/European NIF extractor, and system certificate detection.
3. [x] **Phase 3**: Write integration tests in `crates/kestrel-core/tests/integration_tests.rs`.
4. [x] **Phase 4**: Wire certificate detection and selection into `KestrelApp` in `crates/kestrel-app/src/app.rs`.
5. [x] **Phase 5**: Write E2E smoke tests in `crates/kestrel-app/tests/smoke_test.rs`.
6. [x] **Phase 6**: Verification battery (`cargo fmt`, `cargo clippy`, `cargo test`, `cargo check --target wasm32-unknown-unknown`).
7. [ ] **Phase 7**: PR, CI check, merge to `main`, bump to `v0.3.3`, tag release, and monitor asset publication.

---

## 5. Acceptance Criteria

- [x] Auto-detection of installed system certificates works without crashing or panicking.
- [x] Selecting a certificate immediately autofills `Firmante`, `NIF/CIF`, `Emisor CA`, `Motivo`, and displays validity status.
- [x] Loading certificate files (`.crt`, `.cer`, `.pem`, `.p12`, `.pfx`) also extracts metadata and autofills the form.
- [x] Zero binary PDFs or local files committed to git.
- [x] 100% test battery passes with 0 warnings.
