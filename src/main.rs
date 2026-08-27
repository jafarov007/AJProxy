mod app;
mod browser;
mod models;
mod proxy;
mod theme;
mod ui;

use app::AJProxyApp;
use eframe::NativeOptions;
use egui::Vec2;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Check if invoked in --internal-browser mode (spawns WRY native webview)
    if args.len() >= 3 && args[1] == "--internal-browser" {
        let port: u16 = args[2].parse().unwrap_or(8080);
        let target_url = if args.len() >= 4 { &args[3] } else { "https://jafarov007.github.io/" };
        browser::webview::run_internal_browser_process(port, target_url);
        return Ok(());
    }

    // Ensure Root CA Certificate exists on startup (~/.config/ajproxy/ on Linux/macOS, %APPDATA%\ajproxy\ on Windows)
    if let Err(e) = proxy::cert::ensure_ca_cert_exists() {
        eprintln!("[AJProxy] CA Certificate warning: {}", e);
    } else {
        // Automatically and silently import Root CA to Chrome & Firefox local databases on startup (non-interactive)
        proxy::cert::install_root_ca_to_nss_db();
    }

    // Start background TCP HTTP/HTTPS Proxy Listener on 127.0.0.1:8080
    proxy::listener::start_proxy_server("127.0.0.1".to_string(), 8080);

    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("AJProxy — Professional HTTP Intercepting Proxy")
            .with_inner_size(Vec2::new(1380.0, 880.0))
            .with_min_inner_size(Vec2::new(1024.0, 680.0))
            .with_decorations(false)
            .with_transparent(true),
        ..Default::default()
    };

    eframe::run_native(
        "AJProxy",
        options,
        Box::new(|cc| {
            // Enable image loading for egui_extras
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Box::new(AJProxyApp::new())
        }),
    )
}
