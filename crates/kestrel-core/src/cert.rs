use anyhow::{Context, Result};
use std::path::Path;

/// Origin or storage source of an installed or loaded digital certificate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CertificateSource {
    /// Windows Cryptographic API Certificate Store (e.g. "MY" / Personal).
    WindowsStore(String),
    /// System certificate directory (e.g. /etc/ssl/certs).
    SystemStore(String),
    /// User certificate storage directory (e.g. ~/.config/kestrel/certificates).
    UserDirectory(String),
    /// Loaded from an explicit file path on disk.
    File(String),
    /// In-memory synthetic mock certificate for automated testing.
    SyntheticMock,
}

impl std::fmt::Display for CertificateSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WindowsStore(store) => write!(f, "Almacén de Windows ({})", store),
            Self::SystemStore(path) => write!(f, "Almacén del Sistema ({})", path),
            Self::UserDirectory(path) => write!(f, "Carpeta de Usuario ({})", path),
            Self::File(path) => {
                let name = Path::new(path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.clone());
                write!(f, "Archivo: {}", name)
            }
            Self::SyntheticMock => write!(f, "Certificado Sintético"),
        }
    }
}

/// Metadata extracted from an X.509 digital certificate for automated signing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledCertificate {
    /// Unique identifier / thumbprint (SHA-256 fingerprint).
    pub id: String,
    /// Extracted human-readable signer or organization name.
    pub subject_name: String,
    /// Extracted national identity (NIF, NIE, CIF) or serial number attribute.
    pub subject_id: Option<String>,
    /// Issuing Certificate Authority (CA) Common Name or Organization.
    pub issuer: String,
    /// Validity start date (e.g. "YYYY-MM-DD").
    pub not_before: String,
    /// Validity expiration date (e.g. "YYYY-MM-DD").
    pub not_after: String,
    /// Whether the certificate is currently valid (not expired and not before start date).
    pub is_valid: bool,
    /// Certificate serial number formatted in hexadecimal.
    pub serial_number: String,
    /// SHA-256 fingerprint formatted in lowercase hexadecimal.
    pub sha256_fingerprint: String,
    /// Raw DER-encoded X.509 certificate bytes.
    pub raw_der: Vec<u8>,
    /// Origin source where the certificate was discovered.
    pub source: CertificateSource,
}

impl InstalledCertificate {
    /// Formats a concise label suitable for UI dropdowns and selection lists.
    pub fn display_label(&self) -> String {
        let id_part = match &self.subject_id {
            Some(id) if !id.trim().is_empty() => format!(" ({})", id.trim()),
            _ => String::new(),
        };
        format!("{}{}", self.subject_name, id_part)
    }

    /// Formats a secondary description showing Issuer and validity expiration.
    pub fn subtitle_label(&self) -> String {
        let status = if self.is_valid {
            format!("Válido hasta {}", self.not_after)
        } else {
            format!("Caducado ({})", self.not_after)
        };
        format!("{} · {}", self.issuer, status)
    }
}

/// Extracts a Spanish or European identity number (NIF, NIE, CIF) from certificate attributes.
pub fn extract_spanish_nif_or_id(raw_serial: Option<&str>, common_name: &str) -> Option<String> {
    // 1. Check raw serialNumber attribute (OID 2.5.4.5)
    if let Some(serial) = raw_serial {
        let trimmed = serial.trim();
        if !trimmed.is_empty() {
            // Remove common Spanish eIDAS prefixes (e.g. "IDCES-", "CIF-", "NIE-", "DNI-")
            let stripped = trimmed
                .strip_prefix("IDCES-")
                .or_else(|| trimmed.strip_prefix("idces-"))
                .or_else(|| trimmed.strip_prefix("CIF-"))
                .or_else(|| trimmed.strip_prefix("cif-"))
                .or_else(|| trimmed.strip_prefix("NIE-"))
                .or_else(|| trimmed.strip_prefix("nie-"))
                .or_else(|| trimmed.strip_prefix("DNI-"))
                .or_else(|| trimmed.strip_prefix("dni-"))
                .unwrap_or(trimmed);
            return Some(stripped.trim().to_uppercase());
        }
    }

    // 2. Search for NIF/NIE pattern inside common name (e.g. "GARCIA PEREZ JUAN - 12345678Z")
    let tokens: Vec<&str> = common_name
        .split(|c: char| {
            c.is_whitespace() || c == '-' || c == '(' || c == ')' || c == ':' || c == ','
        })
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    for token in tokens {
        // NIF format: 8 digits + 1 letter (e.g. 12345678Z)
        if token.len() == 9 {
            let bytes = token.as_bytes();
            let digits_ok = bytes[0..8].iter().all(|b| b.is_ascii_digit());
            let letter_ok = bytes[8].is_ascii_alphabetic();
            if digits_ok && letter_ok {
                return Some(token.to_uppercase());
            }

            // NIE format: X/Y/Z + 7 digits + 1 letter (e.g. X1234567A)
            let nie_prefix = bytes[0] == b'X'
                || bytes[0] == b'Y'
                || bytes[0] == b'Z'
                || bytes[0] == b'x'
                || bytes[0] == b'y'
                || bytes[0] == b'z';
            let nie_digits_ok = bytes[1..8].iter().all(|b| b.is_ascii_digit());
            if nie_prefix && nie_digits_ok && letter_ok {
                return Some(token.to_uppercase());
            }

            // CIF format: 1 letter + 7 digits + 1 alphanumeric (e.g. B12345678)
            let cif_prefix = bytes[0].is_ascii_alphabetic();
            let cif_digits_ok = bytes[1..8].iter().all(|b| b.is_ascii_digit());
            let cif_suffix = bytes[8].is_ascii_alphanumeric();
            if cif_prefix && cif_digits_ok && cif_suffix {
                return Some(token.to_uppercase());
            }
        }
    }

    None
}

/// Cleans subject common name by removing trailing NIF/ID, parentheses, or "(FIRMA)" annotations.
pub fn clean_subject_name(common_name: &str, nif: Option<&str>) -> String {
    let mut name = common_name.trim().to_string();

    // If NIF is present at the end separated by hyphen, strip it
    if let Some(id) = nif {
        if let Some(idx) = name.to_uppercase().rfind(id) {
            let before = name[..idx].trim().trim_end_matches('-').trim();
            if !before.is_empty() {
                name = before.to_string();
            }
        }
    }

    // Strip common annotations: "(FIRMA)", "(AUTENTICACIÓN)", "(DNIe)"
    name = name
        .replace("(FIRMA)", "")
        .replace("(firma)", "")
        .replace("(AUTENTICACIÓN)", "")
        .replace("(autenticación)", "")
        .replace("(DNIe)", "")
        .trim()
        .to_string();

    if name.is_empty() {
        common_name.trim().to_string()
    } else {
        name
    }
}

/// Formats an ASN1Time into "YYYY-MM-DD" date string.
fn format_asn1_time(time: &x509_parser::time::ASN1Time) -> String {
    let dt = time.to_datetime();
    format!("{:04}-{:02}-{:02}", dt.year(), dt.month() as u8, dt.day())
}

/// Parses an X.509 certificate from raw DER-encoded bytes.
pub fn parse_x509_der(der_bytes: &[u8], source: CertificateSource) -> Result<InstalledCertificate> {
    let (_, cert) = x509_parser::parse_x509_certificate(der_bytes)
        .map_err(|e| anyhow::anyhow!("Error al decodificar certificado X.509 DER: {:?}", e))?;

    // Extract Subject Common Name, SerialNumber, Organization, etc.
    let mut raw_cn: Option<String> = None;
    let mut raw_serial: Option<String> = None;
    let mut raw_org: Option<String> = None;

    for rdn in cert.subject().iter_rdn() {
        for attr in rdn.iter() {
            if attr.attr_type() == &x509_parser::oid_registry::OID_X509_COMMON_NAME {
                if let Ok(s) = attr.as_str() {
                    raw_cn = Some(s.to_string());
                }
            } else if attr.attr_type() == &x509_parser::oid_registry::OID_X509_SERIALNUMBER {
                if let Ok(s) = attr.as_str() {
                    raw_serial = Some(s.to_string());
                }
            } else if attr.attr_type() == &x509_parser::oid_registry::OID_X509_ORGANIZATION_NAME {
                if let Ok(s) = attr.as_str() {
                    raw_org = Some(s.to_string());
                }
            }
        }
    }

    let raw_name = raw_cn
        .or(raw_org)
        .unwrap_or_else(|| cert.subject().to_string());

    let subject_id = extract_spanish_nif_or_id(raw_serial.as_deref(), &raw_name);
    let subject_name = clean_subject_name(&raw_name, subject_id.as_deref());

    // Extract Issuer Common Name or Organization
    let mut issuer_cn: Option<String> = None;
    let mut issuer_org: Option<String> = None;

    for rdn in cert.issuer().iter_rdn() {
        for attr in rdn.iter() {
            if attr.attr_type() == &x509_parser::oid_registry::OID_X509_COMMON_NAME {
                if let Ok(s) = attr.as_str() {
                    issuer_cn = Some(s.to_string());
                }
            } else if attr.attr_type() == &x509_parser::oid_registry::OID_X509_ORGANIZATION_NAME {
                if let Ok(s) = attr.as_str() {
                    issuer_org = Some(s.to_string());
                }
            }
        }
    }

    let issuer = issuer_cn
        .or(issuer_org)
        .unwrap_or_else(|| cert.issuer().to_string());

    let validity = cert.validity();
    let not_before = format_asn1_time(&validity.not_before);
    let not_after = format_asn1_time(&validity.not_after);
    let is_valid = validity.is_valid();

    let serial_number = cert.raw_serial_as_string();

    let hash = crate::sign::compute_document_sha256(der_bytes);
    let sha256_fingerprint = crate::sign::hex_encode(&hash);
    let id = sha256_fingerprint.clone();

    Ok(InstalledCertificate {
        id,
        subject_name,
        subject_id,
        issuer,
        not_before,
        not_after,
        is_valid,
        serial_number,
        sha256_fingerprint,
        raw_der: der_bytes.to_vec(),
        source,
    })
}

/// Parses X.509 certificates from PEM formatted content (e.g. .crt, .cer, .pem).
pub fn parse_x509_pem(
    pem_bytes: &[u8],
    source: CertificateSource,
) -> Result<Vec<InstalledCertificate>> {
    let mut certs = Vec::new();
    for pem in x509_parser::pem::Pem::iter_from_buffer(pem_bytes) {
        let pem = pem.context("Error leyendo bloque PEM")?;
        if pem.label == "CERTIFICATE" {
            match parse_x509_der(&pem.contents, source.clone()) {
                Ok(c) => certs.push(c),
                Err(e) => tracing::warn!("Omitiendo bloque de certificado PEM inválido: {:?}", e),
            }
        }
    }

    if certs.is_empty() {
        anyhow::bail!("No se encontraron certificados válidos en formato PEM");
    }

    Ok(certs)
}

/// Automatically detects format (PEM vs DER) and parses certificate bytes.
pub fn parse_certificate_bytes(
    bytes: &[u8],
    source: CertificateSource,
) -> Result<Vec<InstalledCertificate>> {
    let trimmed = bytes.trim_ascii();
    if trimmed.starts_with(b"-----BEGIN") {
        parse_x509_pem(trimmed, source)
    } else {
        // Try direct DER parsing
        match parse_x509_der(bytes, source.clone()) {
            Ok(cert) => Ok(vec![cert]),
            Err(e) => {
                // Check if it's a PKCS#12 or container with an embedded X.509 cert
                if let Some(embedded_cert) = find_embedded_x509_in_bytes(bytes, source) {
                    Ok(vec![embedded_cert])
                } else {
                    Err(e)
                }
            }
        }
    }
}

/// Scans arbitrary binary buffer (such as PKCS#12) for embedded X.509 certificate headers.
fn find_embedded_x509_in_bytes(
    bytes: &[u8],
    source: CertificateSource,
) -> Option<InstalledCertificate> {
    if bytes.len() < 64 {
        return None;
    }

    // Look for ASN.1 SEQUENCE tag 0x30 followed by 0x82 (2-byte length)
    for i in 0..bytes.len().saturating_sub(64) {
        if bytes[i] == 0x30 && bytes[i + 1] == 0x82 {
            let len = ((bytes[i + 2] as usize) << 8) | (bytes[i + 3] as usize);
            let total_len = len + 4;
            if i + total_len <= bytes.len() && total_len > 100 {
                if let Ok(cert) = parse_x509_der(&bytes[i..i + total_len], source.clone()) {
                    return Some(cert);
                }
            }
        }
    }
    None
}

/// Automatically detects installed digital certificates in the operating system.
#[cfg(target_os = "windows")]
pub fn detect_system_certificates() -> Vec<InstalledCertificate> {
    let mut certs = Vec::new();
    unsafe {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        // Open Current User "MY" Store (Personal Certificates)
        let store_name: Vec<u16> = OsStr::new("MY").encode_wide().chain(Some(0)).collect();
        let h_store = windows_sys::Win32::Security::Cryptography::CertOpenSystemStoreW(
            0,
            store_name.as_ptr(),
        );

        if h_store.is_null() {
            return certs;
        }

        let mut p_ctx = std::ptr::null();
        loop {
            p_ctx = windows_sys::Win32::Security::Cryptography::CertEnumCertificatesInStore(
                h_store, p_ctx,
            );
            if p_ctx.is_null() {
                break;
            }

            let cert_ctx = &*p_ctx;
            if cert_ctx.cbCertEncoded > 0 && !cert_ctx.pbCertEncoded.is_null() {
                let der_slice = std::slice::from_raw_parts(
                    cert_ctx.pbCertEncoded,
                    cert_ctx.cbCertEncoded as usize,
                );
                if let Ok(cert) = parse_x509_der(
                    der_slice,
                    CertificateSource::WindowsStore("Personal".to_string()),
                ) {
                    certs.push(cert);
                }
            }
        }

        windows_sys::Win32::Security::Cryptography::CertCloseStore(h_store, 0);
    }

    certs
}

/// Automatically detects installed digital certificates in the operating system (Unix / Desktop).
#[cfg(all(not(target_os = "windows"), not(target_arch = "wasm32")))]
pub fn detect_system_certificates() -> Vec<InstalledCertificate> {
    let mut certs = Vec::new();
    let mut search_paths = Vec::new();

    // 1. User certificates in ~/.config/kestrel/certificates
    if let Ok(home) = std::env::var("HOME") {
        let kestrel_dir = std::path::PathBuf::from(&home).join(".config/kestrel/certificates");
        if kestrel_dir.is_dir() {
            search_paths.push((
                kestrel_dir,
                CertificateSource::UserDirectory("~/.config/kestrel/certificates".to_string()),
            ));
        }
        let local_ca = std::path::PathBuf::from(&home).join(".local/share/ca-certificates");
        if local_ca.is_dir() {
            search_paths.push((
                local_ca,
                CertificateSource::UserDirectory("~/.local/share/ca-certificates".to_string()),
            ));
        }
    }

    for (dir, source) in search_paths {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let ext = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|s| s.to_lowercase())
                        .unwrap_or_default();
                    if matches!(ext.as_str(), "crt" | "cer" | "pem" | "p12" | "pfx" | "der") {
                        if let Ok(bytes) = std::fs::read(&path) {
                            if let Ok(parsed) = parse_certificate_bytes(&bytes, source.clone()) {
                                certs.extend(parsed);
                            }
                        }
                    }
                }
            }
        }
    }

    certs
}

/// Safe stub for WebAssembly environments (no local filesystem or OS keystores).
#[cfg(target_arch = "wasm32")]
pub fn detect_system_certificates() -> Vec<InstalledCertificate> {
    Vec::new()
}
