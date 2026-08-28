use std::fs;
use std::process::{Command, Stdio};
use crate::proxy::cert::{get_cert_path, ensure_ca_cert_exists};

/// Installs the Root CA into system-wide trust store (Ubuntu/Debian, Windows, macOS).
pub fn install_ca_system_wide() -> Result<String, String> {
    ensure_ca_cert_exists().map_err(|e| e.to_string())?;
    let cert_path = get_cert_path();

    #[cfg(target_os = "linux")]
    {
        let cmd = format!(
            "pkexec sh -c 'cp \"{}\" /usr/local/share/ca-certificates/ajproxy_ca.crt && update-ca-certificates' || sudo cp \"{}\" /usr/local/share/ca-certificates/ajproxy_ca.crt && sudo update-ca-certificates",
            cert_path.display(), cert_path.display()
        );
        let output = Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .output();

        install_root_ca_to_nss_db();

        match output {
            Ok(out) if out.status.success() => {
                Ok("✅ Root CA successfully installed to Ubuntu system trust store!".to_string())
            }
            _ => {
                Ok("✅ Root CA imported to browser databases!".to_string())
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
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

    #[cfg(target_os = "macos")]
    {
        let output = Command::new("sudo")
            .args(&["security", "add-trusted-cert", "-d", "-r", "trustRoot", "-k", "/Library/Keychains/System.keychain", &cert_path.to_string_lossy()])
            .output();

        match output {
            Ok(out) if out.status.success() => {
                Ok("✅ Root CA successfully installed to macOS System Keychain!".to_string())
            }
            _ => Err("Failed to import Root CA to macOS System Keychain.".to_string())
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        Err("System-wide auto-trust is not supported on this OS.".to_string())
    }
}

/// Automatically install Root CA into Linux NSS Databases (Chrome, Chromium, Firefox) non-interactively.
pub fn install_root_ca_to_nss_db() {
    if cfg!(target_os = "linux") {
        let cert_path = get_cert_path();
        if !cert_path.exists() {
            return;
        }

        let empty_pass_path = std::env::temp_dir().join("ajproxy_empty_pass.txt");
        let empty_pass_file = empty_pass_path.to_str().unwrap_or("/tmp/ajproxy_empty_pass.txt");
        let _ = fs::write(&empty_pass_path, "\n");

        if let Some(home_dir) = home::home_dir() {
            // 1. Chrome / Chromium NSS DB (~/.pki/nssdb)
            let nss_dir = home_dir.join(".pki/nssdb");
            if nss_dir.exists() {
                println!("[AJProxy CA] Attempting automatic Root CA import to Chrome NSS DB (~/.pki/nssdb)...");
                let _ = Command::new("certutil")
                    .args(&[
                        "-d", &format!("sql:{}", nss_dir.to_string_lossy()),
                        "-D",
                        "-n", "AJProxy Root CA",
                        "-f", empty_pass_file,
                    ])
                    .stdin(Stdio::null())
                    .output();

                let _ = Command::new("certutil")
                    .args(&[
                        "-d", &format!("sql:{}", nss_dir.to_string_lossy()),
                        "-A", "-t", "C,,",
                        "-n", "AJProxy Root CA",
                        "-i", &cert_path.to_string_lossy(),
                        "-f", empty_pass_file,
                    ])
                    .stdin(Stdio::null())
                    .output();
            }

            // 2. Modern Chrome / Chromium NSS DB (~/.local/share/pki/nssdb)
            let modern_nss_dir = home_dir.join(".local/share/pki/nssdb");
            if modern_nss_dir.exists() {
                println!("[AJProxy CA] Attempting automatic Root CA import to Chrome modern NSS DB (~/.local/share/pki/nssdb)...");
                let _ = Command::new("certutil")
                    .args(&[
                        "-d", &format!("sql:{}", modern_nss_dir.to_string_lossy()),
                        "-D",
                        "-n", "AJProxy Root CA",
                        "-f", empty_pass_file,
                    ])
                    .stdin(Stdio::null())
                    .output();

                let _ = Command::new("certutil")
                    .args(&[
                        "-d", &format!("sql:{}", modern_nss_dir.to_string_lossy()),
                        "-A", "-t", "C,,",
                        "-n", "AJProxy Root CA",
                        "-i", &cert_path.to_string_lossy(),
                        "-f", empty_pass_file,
                    ])
                    .stdin(Stdio::null())
                    .output();
            }

            // 3. Firefox Profiles (~/.mozilla/firefox/*)
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
                                        "-D",
                                        "-n", "AJProxy Root CA",
                                        "-f", empty_pass_file,
                                    ])
                                    .stdin(Stdio::null())
                                    .output();

                                let _ = Command::new("certutil")
                                    .args(&[
                                        "-d", &format!("sql:{}", path.to_string_lossy()),
                                        "-A", "-t", "C,,",
                                        "-n", "AJProxy Root CA",
                                        "-i", &cert_path.to_string_lossy(),
                                        "-f", empty_pass_file,
                                    ])
                                    .stdin(Stdio::null())
                                    .output();
                            }
                        }
                    }
                }
            }
        }
    }
}
