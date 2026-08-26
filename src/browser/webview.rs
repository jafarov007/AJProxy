use std::process::Command;
use tao::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};
use wry::{ProxyConfig, ProxyEndpoint, WebViewBuilder};

/// Launches the embedded native WebView process (WebKitGTK / WebView2 / WKWebView)
/// by executing `ajproxy --internal-browser <proxy_port>`.
/// This avoids event loop conflicts with eframe and requires NO external browsers installed.
pub fn launch_embedded_browser(proxy_port: u16, initial_url: Option<&str>) -> Result<(), String> {
    let current_exe = std::env::current_exe()
        .map_err(|e| format!("Failed to resolve current binary path: {}", e))?;

    let url = initial_url.unwrap_or("https://httpbin.org/get");

    println!("[AJProxy] Spawning native Webview process: {} --internal-browser {} {}", current_exe.display(), proxy_port, url);

    let child = Command::new(&current_exe)
        .arg("--internal-browser")
        .arg(proxy_port.to_string())
        .arg(url)
        .spawn();

    match child {
        Ok(_) => Ok(()),
        Err(e) => {
            eprintln!("[AJProxy] Process spawn error: {}", e);
            Err(format!("Failed to spawn internal browser: {}", e))
        }
    }
}

/// Runs the native WRY WebView loop on the main thread for the `--internal-browser` mode.
/// - Linux: Uses WebKitGTK
/// - Windows: Uses WebView2
/// - macOS: Uses WKWebView
pub fn run_internal_browser_process(proxy_port: u16, target_url: &str) {
    #[cfg(target_os = "linux")]
    {
        let _ = gtk::init();
    }

    let event_loop = EventLoopBuilder::new().build();
    let window_res = WindowBuilder::new()
        .with_title(format!("AJProxy Internal Browser — Intercepting Port {}", proxy_port))
        .with_inner_size(LogicalSize::new(1180.0, 790.0))
        .build(&event_loop);

    match window_res {
        Ok(window) => {
            let proxy_endpoint = ProxyEndpoint {
                host: "127.0.0.1".to_string(),
                port: proxy_port.to_string(),
            };

            let webview_builder = WebViewBuilder::new(&window)
                .with_url(target_url)
                .with_proxy_config(ProxyConfig::Http(proxy_endpoint));

            match webview_builder.build() {
                Ok(_webview) => {
                    println!("[AJProxy Browser] WRY Native WebView initialized successfully!");
                    event_loop.run(move |event, _, control_flow| {
                        *control_flow = ControlFlow::Wait;
                        if let Event::WindowEvent {
                            event: WindowEvent::CloseRequested,
                            ..
                        } = event
                        {
                            *control_flow = ControlFlow::Exit;
                        }
                    });
                }
                Err(e) => {
                    eprintln!("[AJProxy Browser] WebView creation failed: {}. Falling back to browser spawn.", e);
                    let _ = spawn_fallback_browser(proxy_port, target_url);
                }
            }
        }
        Err(e) => {
            eprintln!("[AJProxy Browser] Window creation failed: {}", e);
            let _ = spawn_fallback_browser(proxy_port, target_url);
        }
    }
}

fn prepare_chrome_profile_trust(proxy_port: u16) {
    let profile_dir = format!("/tmp/ajproxy_chrome_profile_{}", proxy_port);
    let path = std::path::Path::new(&profile_dir);
    std::fs::create_dir_all(path).ok();

    let cert_path = crate::proxy::cert::get_cert_path();
    if cert_path.exists() {
        let _ = Command::new("certutil")
            .args(&["-d", &format!("sql:{}", profile_dir), "-N", "--empty-password"])
            .output();

        let _ = Command::new("certutil")
            .args(&[
                "-d", &format!("sql:{}", profile_dir),
                "-A", "-t", "C,,",
                "-n", "AJProxy Root CA",
                "-i", &cert_path.to_string_lossy(),
            ])
            .output();
    }
}

fn spawn_fallback_browser(proxy_port: u16, target_url: &str) -> Result<(), String> {
    prepare_chrome_profile_trust(proxy_port);
    let proxy_arg = format!("127.0.0.1:{}", proxy_port);

    let mut spki_flag = String::new();
    if let Some(b64) = crate::proxy::cert::get_ca_spki_sha256_base64() {
        spki_flag = format!("--ignore-certificate-errors-spki-list={}", b64);
    }

    let mut chrome_args = vec![
        format!("--proxy-server={}", proxy_arg),
        format!("--user-data-dir=/tmp/ajproxy_chrome_profile_{}", proxy_port),
        "--ignore-certificate-errors".to_string(),
        "--ignore-ssl-errors".to_string(),
        "--test-type".to_string(),
        "--disable-infobars".to_string(),
        "--allow-insecure-localhost".to_string(),
        "--no-first-run".to_string(),
        "--no-default-browser-check".to_string(),
    ];
    if !spki_flag.is_empty() {
        chrome_args.push(spki_flag.clone());
    }
    chrome_args.push(target_url.to_string());

    let mut chromium_args = vec![
        format!("--proxy-server={}", proxy_arg),
        format!("--user-data-dir=/tmp/ajproxy_chromium_profile_{}", proxy_port),
        "--ignore-certificate-errors".to_string(),
        "--ignore-ssl-errors".to_string(),
        "--test-type".to_string(),
        "--disable-infobars".to_string(),
        "--allow-insecure-localhost".to_string(),
        "--no-first-run".to_string(),
        "--no-default-browser-check".to_string(),
    ];
    if !spki_flag.is_empty() {
        chromium_args.push(spki_flag);
    }
    chromium_args.push(target_url.to_string());

    let candidates: &[(&str, Vec<String>)] = &[
        ("google-chrome", chrome_args.clone()),
        ("chromium", chromium_args.clone()),
        ("chromium-browser", chromium_args),
        ("firefox", vec![target_url.to_string()]),
        ("x-www-browser", vec![target_url.to_string()]),
    ];

    for (bin, args) in candidates {
        if Command::new(bin).args(args).spawn().is_ok() {
            println!("[AJProxy Browser] Successfully spawned browser '{}' with zero-config SSL trust flags", bin);
            return Ok(());
        }
    }

    Err("Fallback browser launch failed".into())
}
