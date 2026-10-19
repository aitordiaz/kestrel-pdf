# HANDOFF-0021: System Certificate Auto-Detection & Automated Metadata Extraction

- **Date**: 2026-10-19
- **Author**: Antigravity (AI Pair Programmer)
- **Active Version**: v0.3.3
- **Active Branch**: `feat/auto-detect-system-certificates-and-autofill`

---

## 1. Executive Summary

In response to user request, version `v0.3.3` introduces:
1. **Operating System Digital Certificate Auto-Detection**:
   - Developed `kestrel_core::cert::detect_system_certificates()` to automatically discover personal certificates installed on the host operating system without requiring manual filesystem searching.
   - On Windows: Queries the Windows CryptoAPI "MY" personal store (`CertOpenSystemStoreW`, `CertEnumCertificatesInStore`) via `windows-sys` (`Win32_Security_Cryptography`) to extract user certificates (e.g. FNMT, DNIe, ACA, Camerfirma, corporate certificates).
   - On Unix / Desktop: Scans user certificate directories (`~/.config/kestrel/certificates`, `~/.local/share/ca-certificates`, and custom certificate paths).
   - On WebAssembly: Provides a safe non-panicking stub returning empty lists while allowing in-memory/file loading.
2. **Interactive Certificate Selector**:
   - Added an egui `ComboBox` to the "Firmar con Certificado Digital" modal displaying detected certificates with icon, signer name, NIF, issuer, and validity mark (`✓` / `⚠`).
   - Added re-scan button (`crate::icons::SEARCH` / "Actualizar") to refresh system keystores on demand.
   - Added status pills showing validity (`✓ Vigente hasta YYYY-MM-DD` / `⚠ Caducado el YYYY-MM-DD`) and origin source (`Almacén de Windows (Personal)`, `Carpeta de Usuario`, etc.).
3. **Automated Metadata Extraction & Autofill**:
   - Selecting any certificate immediately extracts and autofills:
     - **Firmante (Signer Name)**: Extracted from Subject `CN` and cleaned of trailing IDs/annotations.
     - **NIF / NIE / CIF**: Extracted from Subject `serialNumber` (OID `2.5.4.5` / `idces-` prefixes) or regex matching in `CN`.
     - **Emisor / CA**: Extracted from Issuer `CN` or Organization (`O`).
     - **Motivo**: Pre-filled with "Documento firmado digitalmente".
   - All fields remain fully editable by the user if customizations are needed.
4. **Enhanced File Loader**:
   - File picker button (`Cargar desde archivo (.p12, .pfx, .crt, .cer)...`) now parses DER, PEM, or PKCS#12 containers, adds the certificate to the list, and immediately triggers full autofill.

---

## 2. Test Coverage & Verification Battery

- **Test Suite Results**: 75 automated tests passing (41 E2E smoke tests + 34 integration tests):
  - `test_integration_x509_metadata_extraction_and_nif_parsing`: Verifies parsing of in-memory synthetic DER and PEM certificates with Spanish NIF and issuer extraction.
  - `test_integration_extract_spanish_nif_or_id_formats`: Verifies NIF/NIE/CIF extraction from OID 2.5.4.5, raw serials with prefixes, and CN formats.
  - `test_e2e_auto_detect_certificates_selection_and_autofill`: Verifies modal rendering with detected system certificates, dropdown selection change, autofill of all input fields, and PAdES document signing.
  - `test_e2e_certificate_bytes_load_and_autofill`: Verifies that loading certificate bytes from a file extracts metadata and updates form fields immediately.
- **Formatting**: `cargo fmt --all -- --check` clean (0 formatting differences).
- **Linter**: `cargo clippy --workspace --all-targets -- -D warnings` clean (0 warnings).
- **Cross-Platform / WASM**: `cargo check -p kestrel-app --target wasm32-unknown-unknown` compiles with 0 errors.

---

## 3. Files Modified & Added

- `crates/kestrel-core/src/cert.rs`: Created module implementing `InstalledCertificate`, `CertificateSource`, `parse_x509_der`, `parse_x509_pem`, `parse_certificate_bytes`, `extract_spanish_nif_or_id`, `clean_subject_name`, and `detect_system_certificates`.
- `crates/kestrel-core/src/lib.rs`: Registered `pub mod cert;`.
- `crates/kestrel-core/Cargo.toml`: Added `x509-parser`, `windows-sys` (target Windows), and `rcgen` (dev-dependency).
- `crates/kestrel-core/tests/integration_tests.rs`: Added unit and integration tests for certificate parsing.
- `crates/kestrel-app/Cargo.toml`: Added `rcgen` (dev-dependency).
- `crates/kestrel-app/src/app.rs`: Added `detected_certificates` and `selected_certificate_index`, selection handlers, and updated modal window with ComboBox and status pills.
- `crates/kestrel-app/tests/smoke_test.rs`: Added E2E tests for certificate selection, autofill, and file loading.
- `Cargo.toml`: Added `x509-parser` to workspace dependencies and bumped version to `0.3.3`.
- `docs/plans/0019-system-certificate-auto-detection-and-metadata-extraction.md`: Authored and completed SDD plan.
- `docs/plans/README.md`: Updated plans index.
- `docs/handoffs/0021-system-certificate-auto-detection-and-metadata-autofill.md`: Created handoff log.
- `docs/handoffs/README.md`: Updated handoffs index.
