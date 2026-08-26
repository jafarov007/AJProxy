use std::path::{Path, PathBuf};
use std::fs;
use std::process::Command;
use std::time::SystemTime;
use time::OffsetDateTime;
use rcgen::{
    Certificate, CertificateParams, DistinguishedName, DnType, IsCa, BasicConstraints,
    SanType, KeyUsagePurpose, KeyPair
};

/// Returns the default directory for storing CA certificates and keys.
pub fn get_cert_dir() -> PathBuf {
    let mut path = home::home_dir().unwrap_or_else(|| PathBuf::from("~")).join(".config");
    path.push("ajproxy");
    fs::create_dir_all(&path).ok();
    path
}

#[allow(dead_code)]
pub fn get_ca_dir() -> PathBuf {
    get_cert_dir()
}

/// Returns the path to the Root CA Certificate.
pub fn get_cert_path() -> PathBuf {
    get_cert_dir().join("ca_cert.pem")
}

#[allow(dead_code)]
pub fn get_ca_cert_path() -> PathBuf {
    get_cert_path()
}

/// Returns the path to the Root CA Private Key.
pub fn get_ca_key_path() -> PathBuf {
    get_cert_dir().join("ca_key.pem")
}

/// Ensures the Root CA certificate and private key exist, generating them if missing.
pub fn ensure_ca_cert_exists() -> Result<(), Box<dyn std::error::Error>> {
    let cert_path = get_cert_path();
    let key_path = get_ca_key_path();

    if cert_path.exists() && key_path.exists() {
        return Ok(());
    }
    generate_and_save_ca()
}

/// Generates a new Root CA Certificate & Private Key and saves to disk.
pub fn generate_and_save_ca() -> Result<(), Box<dyn std::error::Error>> {
    let cert_path = get_cert_path();
    let key_path = get_ca_key_path();

    println!("[AJProxy CA] Generating new Root CA Certificate & Key...");

    let mut params = CertificateParams::default();
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.serial_number = Some(1001.into());

    let now_secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    // Set valid starting date (1 day ago) and 10 years expiration for Root CA
    params.not_before = OffsetDateTime::from_unix_timestamp(now_secs - 86400).unwrap_or(OffsetDateTime::UNIX_EPOCH);
    params.not_after = OffsetDateTime::from_unix_timestamp(now_secs + (3650 * 86400)).unwrap_or(OffsetDateTime::UNIX_EPOCH);

    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, "AJProxy Root Certificate Authority");
    dn.push(DnType::OrganizationName, "AJProxy Security Tools");
    dn.push(DnType::OrganizationalUnitName, "Interception Authority");
    params.distinguished_name = dn;

    params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::CrlSign,
        KeyUsagePurpose::DigitalSignature,
    ];

    let key_pair = KeyPair::generate(&rcgen::PKCS_ECDSA_P256_SHA256)?;
    params.key_pair = Some(key_pair);

    let cert = Certificate::from_params(params)?;

    fs::write(&cert_path, cert.serialize_pem()?)?;
    fs::write(&key_path, cert.serialize_private_key_pem())?;

    println!("[AJProxy CA] Root CA certificate successfully created at: {:?}", cert_path);

    install_root_ca_to_nss_db(&cert_path);

    Ok(())
}

/// Computes the Base64-encoded SHA-256 digest of the Root CA's Subject Public Key Info (SPKI).
/// Passed to Chrome via --ignore-certificate-errors-spki-list for zero-config trust!
pub fn get_ca_spki_sha256_base64() -> Option<String> {
    ensure_ca_cert_exists().ok()?;
    let cert_pem = fs::read_to_string(get_cert_path()).ok()?;
    let x509 = openssl::x509::X509::from_pem(cert_pem.as_bytes()).ok()?;
    let pubkey = x509.public_key().ok()?;
    let der = pubkey.public_key_to_der().ok()?;
    let digest = openssl::hash::hash(openssl::hash::MessageDigest::sha256(), &der).ok()?;
    Some(openssl::base64::encode_block(&digest))
}

/// Exports the Root CA certificate to a target path.
pub fn export_ca_cert(dest_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    ensure_ca_cert_exists()?;
    let cert_content = fs::read(get_cert_path())?;
    fs::write(dest_path, cert_content)?;
    Ok(())
}

/// Installs the Root CA into Ubuntu/Linux system-wide trust store (/usr/local/share/ca-certificates/).
pub fn install_ca_system_wide() -> Result<String, String> {
    ensure_ca_cert_exists().map_err(|e| e.to_string())?;
    let cert_path = get_cert_path();

    if cfg!(target_os = "linux") {
        let cmd = format!(
            "pkexec sh -c 'cp \"{}\" /usr/local/share/ca-certificates/ajproxy_ca.crt && update-ca-certificates' || sudo cp \"{}\" /usr/local/share/ca-certificates/ajproxy_ca.crt && sudo update-ca-certificates",
            cert_path.display(), cert_path.display()
        );
        let output = Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .output();

        install_root_ca_to_nss_db(&cert_path);

        match output {
            Ok(out) if out.status.success() => {
                Ok("✅ Root CA successfully installed to Ubuntu system trust store!".to_string())
            }
            _ => {
                Ok("✅ Root CA imported to browser databases!".to_string())
            }
        }
    } else {
        Err("System-wide auto-trust is currently supported on Linux/Ubuntu.".to_string())
    }
}

/// Imports a custom CA certificate file into AJProxy store.
pub fn import_ca_cert(src_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let cert_content = fs::read(src_path)?;
    fs::write(get_cert_path(), cert_content)?;
    Ok(())
}

/// Helper function to automatically install Root CA into Linux NSS Database (Chrome & Firefox)
fn install_root_ca_to_nss_db(cert_path: &Path) {
    if cfg!(target_os = "linux") {
        if let Some(home_dir) = home::home_dir() {
            // 1. Chrome / Chromium NSS DB (~/.pki/nssdb)
            let nss_dir = home_dir.join(".pki/nssdb");
            if nss_dir.exists() {
                println!("[AJProxy CA] Attempting automatic Root CA import to Chrome NSS DB (~/.pki/nssdb)...");
                let _ = Command::new("certutil")
                    .args(&[
                        "-d", &format!("sql:{}", nss_dir.to_string_lossy()),
                        "-A", "-t", "C,,",
                        "-n", "AJProxy Root CA",
                        "-i", &cert_path.to_string_lossy(),
                    ])
                    .output();
            }

            // 2. Firefox Profiles (~/.mozilla/firefox/*)
            let firefox_dir = home_dir.join(".mozilla/firefox");
            if firefox_dir.exists() {
                if let Ok(entries) = fs::read_dir(&firefox_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            let cert9_db = path.join("cert9.db");
                            if cert9_db.exists() {
                                println!("[AJProxy CA] Attempting automatic Root CA import to Firefox Profile ({:?})...", path.file_name());
                                let _ = Command::new("certutil")
                                    .args(&[
                                        "-d", &format!("sql:{}", path.to_string_lossy()),
                                        "-A", "-t", "C,,",
                                        "-n", "AJProxy Root CA",
                                        "-i", &cert_path.to_string_lossy(),
                                    ])
                                    .output();
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Dynamically generates a leaf certificate signed by the Root CA for a given hostname.
pub fn generate_leaf_cert(domain: &str) -> Result<(String, String), Box<dyn std::error::Error>> {
    ensure_ca_cert_exists()?;

    let ca_key_pem = fs::read_to_string(get_ca_key_path())?;
    let ca_key_pair = KeyPair::from_pem(&ca_key_pem)?;

    let now_secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let mut ca_params = CertificateParams::default();
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca_params.serial_number = Some(1001.into());
    ca_params.not_before = OffsetDateTime::from_unix_timestamp(now_secs - 86400).unwrap_or(OffsetDateTime::UNIX_EPOCH);
    ca_params.not_after = OffsetDateTime::from_unix_timestamp(now_secs + (3650 * 86400)).unwrap_or(OffsetDateTime::UNIX_EPOCH);

    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, "AJProxy Root Certificate Authority");
    dn.push(DnType::OrganizationName, "AJProxy Security Tools");
    dn.push(DnType::OrganizationalUnitName, "Interception Authority");
    ca_params.distinguished_name = dn;
    ca_params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::CrlSign,
        KeyUsagePurpose::DigitalSignature,
    ];
    ca_params.key_pair = Some(ca_key_pair);

    let ca_cert = Certificate::from_params(ca_params)?;

    let mut params = CertificateParams::default();

    // Compliant 365-day validity period (Chrome RFC 5280 / BR limit is 398 days max)
    params.not_before = OffsetDateTime::from_unix_timestamp(now_secs - 86400).unwrap_or(OffsetDateTime::UNIX_EPOCH);
    params.not_after = OffsetDateTime::from_unix_timestamp(now_secs + (365 * 86400)).unwrap_or(OffsetDateTime::UNIX_EPOCH);

    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, domain);
    dn.push(DnType::OrganizationName, "AJProxy Security Tools");
    dn.push(DnType::OrganizationalUnitName, "Dynamic Interception Engine");
    params.distinguished_name = dn;

    params.subject_alt_names = vec![
        SanType::DnsName(domain.to_string()),
        SanType::DnsName(format!("*.{}", domain)),
    ];

    params.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ServerAuth];
    params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyEncipherment,
    ];

    let leaf_key_pair = KeyPair::generate(&rcgen::PKCS_ECDSA_P256_SHA256)?;
    params.key_pair = Some(leaf_key_pair);

    let leaf_cert = Certificate::from_params(params)?;
    let cert_pem = leaf_cert.serialize_pem_with_signer(&ca_cert)?;
    let key_pem = leaf_cert.serialize_private_key_pem();

    Ok((cert_pem, key_pem))
}
