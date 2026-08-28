#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

use crate::proxy::cert::ensure_ca_cert_exists;

/// Installs the Root CA into system-wide trust store for the host OS.
pub fn install_ca_system_wide() -> Result<String, String> {
    ensure_ca_cert_exists().map_err(|e| e.to_string())?;

    #[cfg(target_os = "linux")]
    {
        linux::install_ca()
    }

    #[cfg(target_os = "windows")]
    {
        windows::install_ca()
    }

    #[cfg(target_os = "macos")]
    {
        macos::install_ca()
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        Err("System-wide auto-trust is not supported on this OS.".to_string())
    }
}

/// Automatically install Root CA into browser NSS Databases (Chrome, Chromium, Firefox) non-interactively.
pub fn install_root_ca_to_nss_db() {
    #[cfg(target_os = "linux")]
    {
        linux::install_to_nss_db();
    }
}
