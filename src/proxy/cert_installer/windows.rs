use std::process::Command;
use crate::proxy::cert::get_cert_path;

/// Windows-specific Root CA installation (certutil -addstore ROOT)
pub fn install_ca() -> Result<String, String> {
    let cert_path = get_cert_path();
    let output = Command::new("certutil")
        .args(&["-addstore", "-f", "ROOT", &cert_path.to_string_lossy()])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            Ok("✅ Root CA successfully installed to Windows Root Certificate Store!".to_string())
        }
        _ => Err("Failed to import Root CA to Windows Store. Please run as Administrator or import manually.".to_string())
    }
}
