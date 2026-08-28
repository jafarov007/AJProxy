use std::process::Command;
use crate::proxy::cert::get_cert_path;

/// macOS-specific Root CA installation (security add-trusted-cert into System.keychain)
pub fn install_ca() -> Result<String, String> {
    let cert_path = get_cert_path();
    let output = Command::new("sudo")
        .args(&[
            "security",
            "add-trusted-cert",
            "-d",
            "-r",
            "trustRoot",
            "-k",
            "/Library/Keychains/System.keychain",
            &cert_path.to_string_lossy(),
        ])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            Ok("✅ Root CA successfully installed to macOS System Keychain!".to_string())
        }
        _ => Err("Failed to import Root CA to macOS System Keychain.".to_string())
    }
}
