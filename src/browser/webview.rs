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
    let url = initial_url.unwrap_or("https://jafarov007.github.io/");

    println!("[AJProxy] Attempting to spawn sandboxed browser (Chrome/Chromium/Firefox) with zero-config SSL trust...");
    if let Ok(_) = spawn_fallback_browser(proxy_port, url) {
        return Ok(());
    }

    println!("[AJProxy] Sandboxed browser launch failed. Falling back to spawning native Webview process...");
    let current_exe = std::env::current_exe()
        .map_err(|e| format!("Failed to resolve current binary path: {}", e))?;

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

            #[cfg(target_os = "linux")]
            let webview_builder = {
                use tao::platform::unix::WindowExtUnix;
                use wry::WebViewBuilderExtUnix;
                let vbox = window.default_vbox().expect("Failed to get default vbox");
                WebViewBuilder::new_gtk(vbox)
            }
            .with_url(target_url)
            .with_proxy_config(ProxyConfig::Http(proxy_endpoint));

            #[cfg(not(target_os = "linux"))]
            let webview_builder = WebViewBuilder::new(&window)
                .with_url(target_url)
                .with_proxy_config(ProxyConfig::Http(proxy_endpoint));

            let build_result = webview_builder.build();

            match build_result {
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

fn spawn_fallback_browser(proxy_port: u16, target_url: &str) -> Result<(), String> {
    let profile_dir = if let Some(home_dir) = home::home_dir() {
        home_dir.join(format!(".config/ajproxy/chrome_profile_{}", proxy_port))
    } else {
        std::env::temp_dir().join(format!("ajproxy_chrome_profile_{}", proxy_port))
    };
    std::fs::create_dir_all(&profile_dir).ok();
    let profile_dir_str = profile_dir.to_string_lossy().to_string();

    let proxy_arg = format!("127.0.0.1:{}", proxy_port);

    let common_args = vec![
        format!("--proxy-server={}", proxy_arg),
        format!("--user-data-dir={}", profile_dir_str),
        "--no-first-run".to_string(),
        "--no-default-browser-check".to_string(),
        "--disable-quic".to_string(),
        "--disable-encrypted-client-hello".to_string(),
        target_url.to_string(),
    ];

    let candidates: &[(&str, Vec<String>)] = &[
        // Linux / Unix binaries
        ("google-chrome", common_args.clone()),
        ("chromium", common_args.clone()),
        ("chromium-browser", common_args.clone()),
        ("firefox", vec![target_url.to_string()]),
        ("x-www-browser", vec![target_url.to_string()]),
        // Windows binaries
        ("chrome.exe", common_args.clone()),
        ("msedge.exe", common_args.clone()),
        ("C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe", common_args.clone()),
        ("C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe", common_args.clone()),
        ("C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe", common_args.clone()),
        // macOS binaries
        ("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome", common_args.clone()),
    ];

    for (bin, args) in candidates {
        if Command::new(bin)
            .args(args)
            .stderr(std::process::Stdio::null())
            .spawn()
            .is_ok()
        {
            println!("[AJProxy Browser] Successfully spawned browser '{}' with zero-config SSL trust flags", bin);
            return Ok(());
        }
    }

    Err("Fallback browser launch failed".into())
}
