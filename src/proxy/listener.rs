use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Instant;

use crate::proxy::cert;
use crate::proxy::http_stream::read_full_http_request;
use crate::proxy::mitm::handle_https_connect_mitm;
use crate::proxy::forwarder::forward_http_request;
pub use crate::proxy::store::*;

static PROXY_RUNNING: AtomicBool = AtomicBool::new(false);

lazy_static::lazy_static! {
    pub static ref UPSTREAM_AGENT: ureq::Agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(60))
        .max_idle_connections(100)
        .max_idle_connections_per_host(10)
        .build();
}

#[allow(dead_code)]
pub fn is_proxy_running() -> bool {
    PROXY_RUNNING.load(Ordering::Relaxed)
}

pub fn start_proxy_server(host: String, port: u16) {
    let addr = format!("{}:{}", host, port);
    let _ = start_proxy_listener(&addr);
}

/// Starts the proxy listener on 127.0.0.1:8080
pub fn start_proxy_listener(addr: &str) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr)?;
    PROXY_RUNNING.store(true, Ordering::Relaxed);
    println!("[AJProxy Engine] Listening on http://{}", addr);

    thread::spawn(move || {
        for stream in listener.incoming() {
            if !PROXY_RUNNING.load(Ordering::Relaxed) {
                break;
            }
            match stream {
                Ok(client_stream) => {
                    thread::spawn(move || {
                        handle_client(client_stream);
                    });
                }
                Err(e) => {
                    eprintln!("[AJProxy Engine] Accept error: {}", e);
                }
            }
        }
    });

    Ok(())
}

fn handle_client(mut client_stream: TcpStream) {
    let start_time = Instant::now();

    let (req_headers, req_body_bytes) = match read_full_http_request(&mut client_stream) {
        Ok(res) if !res.0.is_empty() => res,
        _ => return,
    };
    let req_body = String::from_utf8_lossy(&req_body_bytes).to_string();

    let first_line = req_headers.lines().next().unwrap_or("");

    // ── HTTP / cert Root CA Download Route ─────────────────────────────────
    if first_line.contains("GET /cert ") {
        if let Ok(ca_pem) = std::fs::read_to_string(cert::get_cert_path()) {
            let resp = format!(
                "HTTP/1.1 200 OK\r\n\
                 Content-Type: application/x-pem-file\r\n\
                 Content-Disposition: attachment; filename=\"ajproxy_ca.crt\"\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\r\n{}",
                ca_pem.len(),
                ca_pem
            );
            let _ = client_stream.write_all(resp.as_bytes());
        } else {
            let resp = "HTTP/1.1 500 Internal Server Error\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nRoot CA Certificate not found.";
            let _ = client_stream.write_all(resp.as_bytes());
        }
        return;
    }

    // ── Local Interceptor Landing Page on 127.0.0.1:8080 ──────────────────
    if first_line.contains("GET / HTTP/") && (req_headers.contains("Host: 127.0.0.1") || req_headers.contains("Host: localhost")) {
        let cert_button = if cert::get_cert_path().exists() {
            "<span class=\"badge green\">✔ Root CA Installed & Trusted</span>"
        } else {
            "<a href=\"/cert\" class=\"btn\">📥 Download & Install Root CA Certificate (.crt)</a>"
        };

        let html = format!(
            "<!DOCTYPE html>\n<html>\n<head><title>AJProxy Interceptor Active</title>\n\
            <style>\n\
            body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif; background: #0f172a; color: #f8fafc; text-align: center; padding: 60px 20px; }}\n\
            .card {{ background: #1e293b; max-width: 580px; margin: 0 auto; padding: 40px; border-radius: 16px; box-shadow: 0 10px 25px rgba(0,0,0,0.5); border: 1px solid #334155; }}\n\
            h1 {{ color: #38bdf8; font-size: 28px; margin-bottom: 12px; }}\n\
            p {{ color: #94a3b8; font-size: 15px; line-height: 1.6; }}\n\
            .status {{ inline-block; background: #0284c7; color: white; padding: 6px 14px; border-radius: 20px; font-weight: 600; font-size: 13px; margin: 15px 0; }}\n\
            .btn {{ display: inline-block; background: #10b981; color: white; text-decoration: none; padding: 12px 24px; border-radius: 8px; font-weight: 600; margin-top: 20px; transition: background 0.2s; }}\n\
            .btn:hover {{ background: #059669; }}\n\
            .badge {{ display: inline-block; padding: 8px 16px; border-radius: 6px; font-weight: 600; margin-top: 15px; }}\n\
            .badge.green {{ background: #064e3b; color: #34d399; border: 1px solid #059669; }}\n\
            </style></head>\n\
            <body>\n\
            <div class=\"card\">\n\
            <h1>⚡ AJProxy Interceptor Active</h1>\n\
            <div class=\"status\">PROXY LISTENER RUNNING</div>\n\
            <p>Your browser traffic is being proxied through <strong>AJProxy (127.0.0.1:8080)</strong>.</p>\n\
            <p>All HTTP/HTTPS requests are being recorded and intercepted in real-time.</p>\n\
            <div style=\"margin-top: 25px;\">{}</div>\n\
            </div>\n\
            </body></html>",
            cert_button
        );

        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            html.len(),
            html
        );
        let _ = client_stream.write_all(resp.as_bytes());
        return;
    }

    if first_line.starts_with("CONNECT ") {
        handle_https_connect_mitm(client_stream, &req_headers, start_time);
    } else {
        forward_http_request(client_stream, &req_headers, &req_body, start_time);
    }
}
