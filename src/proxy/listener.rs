use std::collections::HashMap;
use std::io::Write;
use std::net::{TcpListener, TcpStream, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::models::ProxyListenerConfig;
use crate::proxy::cert;
use crate::proxy::http_stream::read_full_http_request;
use crate::proxy::mitm::handle_https_connect_mitm;
use crate::proxy::forwarder::forward_http_request;
pub use crate::proxy::store::*;

lazy_static::lazy_static! {
    pub static ref UPSTREAM_AGENT: ureq::Agent = {
        let tls_connector = native_tls::TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap_or_else(|_| native_tls::TlsConnector::new().unwrap());

        ureq::AgentBuilder::new()
            .tls_connector(Arc::new(tls_connector))
            .timeout(Duration::from_secs(60))
            .max_idle_connections(100)
            .max_idle_connections_per_host(10)
            .build()
    };

    static ref ACTIVE_LISTENERS: Mutex<HashMap<String, Arc<AtomicBool>>> = Mutex::new(HashMap::new());
}

pub fn get_local_ip() -> String {
    if let Ok(socket) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = socket.local_addr() {
                return addr.ip().to_string();
            }
        }
    }
    "127.0.0.1".into()
}

pub fn is_listener_running(bind_address: &str, port: u16) -> bool {
    let addr = format!("{}:{}", bind_address, port);
    if let Ok(listeners) = ACTIVE_LISTENERS.lock() {
        if let Some(flag) = listeners.get(&addr) {
            return flag.load(Ordering::Relaxed);
        }
    }
    false
}

#[allow(dead_code)]
pub fn start_proxy_server(host: String, port: u16) {
    let addr = format!("{}:{}", host, port);
    let _ = start_single_listener(&addr);
}

pub fn sync_listeners(configs: &[ProxyListenerConfig]) {
    let mut listeners = ACTIVE_LISTENERS.lock().unwrap();

    // 1. Identify which addresses should be running, deduplicating ports (prefer 0.0.0.0 over 127.0.0.1 on same port)
    let mut desired_map: HashMap<String, bool> = HashMap::new();
    let mut used_ports: HashMap<u16, String> = HashMap::new();

    // First pass: register valid 0.0.0.0 addresses
    for cfg in configs {
        let trimmed_ip = cfg.bind_address.trim();
        if cfg.enabled && trimmed_ip == "0.0.0.0" {
            let addr = format!("{}:{}", trimmed_ip, cfg.bind_port);
            desired_map.insert(addr.clone(), true);
            used_ports.insert(cfg.bind_port, addr);
        }
    }

    // Second pass: register other valid IP addresses if port not already taken by 0.0.0.0
    for cfg in configs {
        let trimmed_ip = cfg.bind_address.trim();
        if cfg.enabled && trimmed_ip != "0.0.0.0" && trimmed_ip.parse::<std::net::IpAddr>().is_ok() {
            let addr = format!("{}:{}", trimmed_ip, cfg.bind_port);
            if !used_ports.contains_key(&cfg.bind_port) {
                desired_map.insert(addr.clone(), true);
                used_ports.insert(cfg.bind_port, addr);
            }
        }
    }

    // 2. Stop listeners that are no longer desired and wake up thread to free socket immediately
    let current_keys: Vec<String> = listeners.keys().cloned().collect();
    for addr in current_keys {
        if !desired_map.contains_key(&addr) {
            if let Some(flag) = listeners.remove(&addr) {
                flag.store(false, Ordering::Relaxed);
                println!("[AJProxy Engine] Stopping proxy listener on {}", addr);

                // Send a quick dummy loopback connection to unblock listener.incoming() so OS frees the port immediately
                let target = if addr.starts_with("0.0.0.0") {
                    format!("127.0.0.1{}", &addr[7..])
                } else {
                    addr.clone()
                };

                if let Ok(target_sa) = target.parse::<SocketAddr>() {
                    let _ = TcpStream::connect_timeout(&target_sa, Duration::from_millis(50));
                }
            }
        }
    }

    // Give OS a tiny 30ms window to release port
    thread::sleep(Duration::from_millis(30));

    // 3. Start newly desired listeners
    for addr in desired_map.keys() {
        if !listeners.contains_key(addr) {
            let running_flag = Arc::new(AtomicBool::new(true));
            match start_listener_thread(addr.clone(), running_flag.clone()) {
                Ok(_) => {
                    listeners.insert(addr.clone(), running_flag);
                    println!("[AJProxy Engine] Dynamically started proxy listener on http://{}", addr);
                }
                Err(e) => {
                    eprintln!("[AJProxy Engine] Failed to bind proxy listener on {}: {}", addr, e);
                }
            }
        }
    }
}

#[allow(dead_code)]
pub fn start_single_listener(addr: &str) -> std::io::Result<()> {
    let mut listeners = ACTIVE_LISTENERS.lock().unwrap();
    if listeners.contains_key(addr) {
        return Ok(());
    }

    let running_flag = Arc::new(AtomicBool::new(true));
    start_listener_thread(addr.to_string(), running_flag.clone())?;
    listeners.insert(addr.to_string(), running_flag);
    Ok(())
}

fn create_reuse_listener(addr_str: &str) -> std::io::Result<TcpListener> {
    use socket2::{Socket, Domain, Type, Protocol};
    let addr: SocketAddr = addr_str.parse().map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("Invalid address {}: {}", addr_str, e))
    })?;

    let domain = if addr.is_ipv4() { Domain::IPV4 } else { Domain::IPV6 };
    let socket = Socket::new(domain, Type::STREAM, Some(Protocol::TCP))?;

    let _ = socket.set_reuse_address(true);

    #[cfg(target_os = "linux")]
    {
        use std::os::unix::io::AsRawFd;
        let optval: libc::c_int = 1;
        unsafe {
            libc::setsockopt(
                socket.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_REUSEPORT,
                &optval as *const _ as *const libc::c_void,
                std::mem::size_of::<libc::c_int>() as libc::socklen_t,
            );
        }
    }

    let _ = socket.set_nonblocking(false);

    socket.bind(&addr.into())?;
    socket.listen(128)?;

    Ok(socket.into())
}

fn start_listener_thread(addr: String, running_flag: Arc<AtomicBool>) -> std::io::Result<()> {
    let listener = create_reuse_listener(&addr)?;
    println!("[AJProxy Engine] Listening socket active on http://{}", addr);

    thread::spawn(move || {
        for stream in listener.incoming() {
            if !running_flag.load(Ordering::Relaxed) {
                break;
            }
            match stream {
                Ok(client_stream) => {
                    if !running_flag.load(Ordering::Relaxed) {
                        break;
                    }
                    thread::spawn(move || {
                        handle_client(client_stream);
                    });
                }
                Err(e) => {
                    eprintln!("[AJProxy Engine] Accept error on {}: {}", addr, e);
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
    let path_part = first_line.split_whitespace().nth(1).unwrap_or("");

    let host_hdr = req_headers.lines().find_map(|l| {
        if l.to_lowercase().starts_with("host:") {
            l.split_once(':').map(|(_, h)| h.trim())
        } else {
            None
        }
    }).unwrap_or("");

    let is_direct_local = host_hdr.contains("127.0.0.1")
        || host_hdr.contains("localhost")
        || host_hdr.contains("0.0.0.0")
        || host_hdr.contains("192.168.")
        || host_hdr.contains(":8080")
        || path_part.contains("ajproxy");

    if is_direct_local {
        // ── HTTP / cert Root CA Download Route ─────────────────────────────────
        if path_part.ends_with("/cert") || path_part == "/cert" || first_line.contains("/cert") {
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

        // ── Local Favicon Route ───────────────────────────────────────────────
        if path_part.contains("favicon.ico") {
            let resp = "HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n";
            let _ = client_stream.write_all(resp.as_bytes());
            return;
        }

        // ── Local Interceptor Landing Page ──────────────────────────────────────
        if path_part == "/" || path_part.ends_with(":8080/") || first_line.contains("ajproxy") {
            let cert_button = if cert::get_cert_path().exists() {
                "<span class=\"badge green\">✔ Root CA Installed & Trusted</span><br><br><a href=\"/cert\" class=\"btn\">📥 Download Root CA Certificate (.crt)</a>"
            } else {
                "<a href=\"/cert\" class=\"btn\">📥 Download & Install Root CA Certificate (.crt)</a>"
            };

            let html = format!(
                "<!DOCTYPE html>\n<html>\n<head><meta charset=\"UTF-8\"><title>AJProxy Interceptor Active</title>\n\
                <style>\n\
                body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif; background: #0f172a; color: #f8fafc; text-align: center; padding: 60px 20px; }}\n\
                .card {{ background: #1e293b; max-width: 580px; margin: 0 auto; padding: 40px; border-radius: 16px; box-shadow: 0 10px 25px rgba(0,0,0,0.5); border: 1px solid #334155; }}\n\
                h1 {{ color: #38bdf8; font-size: 28px; margin-bottom: 12px; }}\n\
                p {{ color: #94a3b8; font-size: 15px; line-height: 1.6; }}\n\
                .status {{ display: inline-block; background: #0284c7; color: white; padding: 6px 14px; border-radius: 20px; font-weight: 600; font-size: 13px; margin: 15px 0; }}\n\
                .btn {{ display: inline-block; background: #10b981; color: white; text-decoration: none; padding: 12px 24px; border-radius: 8px; font-weight: 600; margin-top: 20px; transition: background 0.2s; }}\n\
                .btn:hover {{ background: #059669; }}\n\
                .badge {{ display: inline-block; padding: 8px 16px; border-radius: 6px; font-weight: 600; margin-top: 15px; }}\n\
                .badge.green {{ background: #064e3b; color: #34d399; border: 1px solid #059669; }}\n\
                </style></head>\n\
                <body>\n\
                <div class=\"card\">\n\
                <h1>⚡ AJProxy Interceptor Active</h1>\n\
                <div class=\"status\">PROXY LISTENER RUNNING</div>\n\
                <p>Your browser traffic is being proxied through <strong>AJProxy</strong>.</p>\n\
                <p>All HTTP/HTTPS requests are being recorded and intercepted in real-time.</p>\n\
                <div style=\"margin-top: 25px;\">{}</div>\n\
                </div>\n\
                </body></html>",
                cert_button
            );

            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                html.as_bytes().len(),
                html
            );
            let _ = client_stream.write_all(resp.as_bytes());
            return;
        }
    }

    if first_line.starts_with("CONNECT ") {
        handle_https_connect_mitm(client_stream, &req_headers, start_time);
    } else {
        forward_http_request(client_stream, &req_headers, &req_body, start_time);
    }
}
