use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use openssl::pkey::PKey;
use openssl::ssl::{SslAcceptor, SslMethod};
use openssl::x509::X509;

use crate::proxy::cert;
use crate::proxy::filters::{apply_match_replace_rules, is_filtered_noise_request, is_passthrough_domain};
use crate::proxy::http_stream::{read_full_http_request, process_and_send_response};
use crate::proxy::websocket::{is_websocket_upgrade, handle_tls_websocket_tunnel, handle_plain_websocket_tunnel};
pub use crate::proxy::store::*;

static PROXY_RUNNING: AtomicBool = AtomicBool::new(false);

lazy_static::lazy_static! {
    static ref UPSTREAM_AGENT: ureq::Agent = ureq::AgentBuilder::new()
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

/// Full TLS MITM Interception Handler with HTTP Keep-Alive
fn handle_https_connect_mitm(mut client_stream: TcpStream, request_str: &str, _start_time: Instant) {
    let first_line = request_str.lines().next().unwrap_or("");
    let target = first_line
        .strip_prefix("CONNECT ")
        .and_then(|s| s.split_whitespace().next())
        .unwrap_or("");

    if target.is_empty() {
        return;
    }

    let target_host = target.split(':').next().unwrap_or(target).to_string();

    // ── Direct TCP Passthrough for Video streaming CDNs & SSL passthrough hosts ──
    if is_passthrough_domain(&target_host) {
        let ack = "HTTP/1.1 200 Connection Established\r\nProxy-Agent: AJProxy/0.1\r\n\r\n";
        if client_stream.write_all(ack.as_bytes()).is_err() {
            return;
        }
        let target_addr = if target.contains(':') { target.to_string() } else { format!("{}:443", target) };
        if let Ok(mut server_stream) = TcpStream::connect(&target_addr) {
            let mut client_clone = match client_stream.try_clone() {
                Ok(c) => c,
                Err(_) => return,
            };
            let mut server_clone = match server_stream.try_clone() {
                Ok(s) => s,
                Err(_) => return,
            };
            thread::spawn(move || {
                let _ = std::io::copy(&mut client_stream, &mut server_stream);
            });
            let _ = std::io::copy(&mut server_clone, &mut client_clone);
        }
        return;
    }

    // Send 200 Connection Established ACK
    let ack = "HTTP/1.1 200 Connection Established\r\nProxy-Agent: AJProxy/0.1\r\n\r\n";
    if client_stream.write_all(ack.as_bytes()).is_err() {
        return;
    }

    // Set socket-level timeouts for keep-alive lifecycle
    let _ = client_stream.set_read_timeout(Some(std::time::Duration::from_secs(120)));
    let _ = client_stream.set_write_timeout(Some(std::time::Duration::from_secs(30)));

    // Generate leaf cert
    let (cert_pem, key_pem) = match cert::generate_leaf_cert(&target_host) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[AJProxy MITM] Cert generation failed for {}: {}", target_host, e);
            return;
        }
    };

    let x509 = match X509::from_pem(cert_pem.as_bytes()) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("[AJProxy MITM] X509 parse error: {}", e);
            return;
        }
    };

    let pkey = match PKey::private_key_from_pem(key_pem.as_bytes()) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("[AJProxy MITM] Private key parse error: {}", e);
            return;
        }
    };

    let mut builder = match SslAcceptor::mozilla_intermediate(SslMethod::tls()) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[AJProxy MITM] SSL Acceptor builder error: {}", e);
            return;
        }
    };

    if let Err(e) = builder.set_private_key(&pkey) {
        eprintln!("[AJProxy MITM] Set private key error: {}", e);
        return;
    }

    if let Err(e) = builder.set_certificate(&x509) {
        eprintln!("[AJProxy MITM] Set certificate error: {}", e);
        return;
    }

    let acceptor = builder.build();

    let mut tls_stream = match acceptor.accept(client_stream) {
        Ok(s) => s,
        Err(e) => {
            let err_msg = e.to_string();
            if !err_msg.contains("unexpected EOF") && !err_msg.contains("Connection reset by peer") {
                eprintln!("[AJProxy MITM] Handshake failed with browser for {}: {}", target_host, e);
            }
            return;
        }
    };

    // ── Keep-alive loop: handle multiple requests on same TLS connection ──
    loop {
        let request_start = Instant::now();

        let (req_headers, req_body_bytes) = match read_full_http_request(&mut tls_stream) {
            Ok(res) if !res.0.is_empty() => res,
            _ => break, // Connection closed or timeout → exit loop
        };
        let req_body = String::from_utf8_lossy(&req_body_bytes).to_string();
        let (req_headers, req_body) = apply_match_replace_rules(req_headers, req_body);

        let first_line = req_headers.lines().next().unwrap_or("");
        let parts: Vec<&str> = first_line.split_whitespace().collect();
        if parts.len() < 2 {
            break;
        }

        let method = parts[0].to_uppercase();
        let raw_path = parts[1];

        let full_url = if raw_path.starts_with("http://") || raw_path.starts_with("https://") {
            raw_path.to_string()
        } else {
            format!("https://{}{}", target_host, raw_path)
        };

        // ── PAUSE IF INTERCEPT IS ON! ─────────────────────────────────────
        if is_intercept_enabled() && !is_filtered_noise_request(&full_url, raw_path, &req_headers) {
            let (tx, rx) = channel();
            let entry_id = next_entry_id();

            let pending = PendingIntercept {
                id: entry_id,
                method: method.clone(),
                host: target_host.clone(),
                path: raw_path.to_string(),
                url: full_url.clone(),
                headers: req_headers.to_string(),
                body: req_body.to_string(),
                responder: Arc::new(Mutex::new(Some(tx))),
            };

            if let Ok(mut lock) = PENDING_INTERCEPTS.lock() {
                lock.push(pending);
            }

            match rx.recv() {
                Ok(InterceptDecision::Forward) => {}
                _ => {
                    let drop_resp = "HTTP/1.1 502 Bad Gateway\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nRequest dropped by AJProxy Interceptor.";
                    let _ = tls_stream.write_all(drop_resp.as_bytes());
                    break;
                }
            }
        }

        // ── WebSocket Upgrade Handling ────────────────────────────────────
        if is_websocket_upgrade(&req_headers) {
            if handle_tls_websocket_tunnel(tls_stream, &target_host, raw_path, &full_url, &method, &req_headers, &req_body, &req_body_bytes, request_start) {
                return;
            }
            break;
        }

        let mut req = UPSTREAM_AGENT.request(&method, &full_url);

        for line in req_headers.lines().skip(1) {
            if let Some((k, v)) = line.split_once(':') {
                let k = k.trim();
                let v = v.trim();
                if !k.eq_ignore_ascii_case("Proxy-Connection") && !k.eq_ignore_ascii_case("Host") {
                    if k.eq_ignore_ascii_case("Accept-Encoding") {
                        req = req.set(k, "gzip, deflate");
                    } else {
                        req = req.set(k, v);
                    }
                }
            }
        }

        let send_res = if !req_body_bytes.is_empty() {
            req.send_bytes(&req_body_bytes)
        } else {
            req.call()
        };

        match send_res {
            Ok(resp) => {
                process_and_send_response(&mut tls_stream, resp, &method, &target_host, raw_path, &full_url, &req_headers, &req_body, request_start);
            }
            Err(ureq::Error::Status(_, resp)) => {
                process_and_send_response(&mut tls_stream, resp, &method, &target_host, raw_path, &full_url, &req_headers, &req_body, request_start);
            }
            Err(e) => {
                let err_str = e.to_string();
                let is_noisy = full_url.contains("android.clients.google.com/checkin")
                    || full_url.contains("clients1.google.com")
                    || full_url.contains("clients2.google.com")
                    || full_url.contains("update.googleapis.com")
                    || full_url.contains("localhost.sensic.net")
                    || full_url.contains("omnitagjs.com");

                if !is_noisy {
                    eprintln!("[AJProxy MITM] Upstream request failed for {}: {}", full_url, err_str);
                }

                let err_resp = format!(
                    "HTTP/1.1 502 Bad Gateway\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nAJProxy Upstream Error: {}",
                    err_str
                );
                let _ = tls_stream.write_all(err_resp.as_bytes());
                let _ = tls_stream.flush();
                break;
            }
        }
    }
}

fn forward_http_request(mut client_stream: TcpStream, req_headers: &str, req_body: &str, start_time: Instant) {
    let (req_headers, req_body) = apply_match_replace_rules(req_headers.to_string(), req_body.to_string());

    if let Some(first_line) = req_headers.lines().next() {
        let parts: Vec<&str> = first_line.split_whitespace().collect();
        if parts.len() >= 2 {
            let method = parts[0].to_uppercase();
            let raw_url = parts[1];

            let mut host_header = String::new();
            for line in req_headers.lines() {
                if line.to_lowercase().starts_with("host:") {
                    if let Some((_, h)) = line.split_once(':') {
                        host_header = h.trim().to_string();
                    }
                }
            }

            let full_url = if raw_url.starts_with("http://") || raw_url.starts_with("https://") {
                raw_url.to_string()
            } else if !host_header.is_empty() {
                format!("http://{}{}", host_header, raw_url)
            } else {
                String::new()
            };

            if !full_url.is_empty() {
                let parsed_url = url::Url::parse(&full_url).ok();
                let host = parsed_url.as_ref().and_then(|u| u.host_str()).unwrap_or(&host_header).to_string();
                let path = parsed_url.as_ref().map(|u| u.path()).unwrap_or(raw_url).to_string();

                // ── PAUSE IF INTERCEPT IS ON! ─────────────────────────────────────
                if is_intercept_enabled() && !is_filtered_noise_request(&full_url, &path, &req_headers) {
                    let (tx, rx) = channel();
                    let entry_id = next_entry_id();

                    let pending = PendingIntercept {
                        id: entry_id,
                        method: method.clone(),
                        host: host.clone(),
                        path: path.clone(),
                        url: full_url.clone(),
                        headers: req_headers.to_string(),
                        body: req_body.to_string(),
                        responder: Arc::new(Mutex::new(Some(tx))),
                    };

                    if let Ok(mut lock) = PENDING_INTERCEPTS.lock() {
                        lock.push(pending);
                    }

                    match rx.recv() {
                        Ok(InterceptDecision::Forward) => {}
                        _ => {
                            let drop_resp = "HTTP/1.1 502 Bad Gateway\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nRequest dropped by AJProxy Interceptor.";
                            let _ = client_stream.write_all(drop_resp.as_bytes());
                            return;
                        }
                    }
                }

                // ── Plain HTTP WebSocket Upgrade Handling ────────────────────────
                if is_websocket_upgrade(&req_headers) {
                    if handle_plain_websocket_tunnel(&mut client_stream, &host, &path, &full_url, &method, &req_headers, &req_body, start_time) {
                        return;
                    }
                }

                let mut req = UPSTREAM_AGENT.request(&method, &full_url);

                for line in req_headers.lines().skip(1) {
                    if let Some((k, v)) = line.split_once(':') {
                        let k = k.trim();
                        let v = v.trim();
                        if !k.eq_ignore_ascii_case("Proxy-Connection") && !k.eq_ignore_ascii_case("Host") {
                            if k.eq_ignore_ascii_case("Accept-Encoding") {
                                req = req.set(k, "gzip, deflate");
                            } else {
                                req = req.set(k, v);
                            }
                        }
                    }
                }

                let send_res = if !req_body.is_empty() {
                    req.send_bytes(req_body.as_bytes())
                } else {
                    req.call()
                };

                let resp_opt = match send_res {
                    Ok(r) => Some(r),
                    Err(ureq::Error::Status(_, r)) => Some(r),
                    Err(e) => {
                        eprintln!("[AJProxy Forwarder] Request error for {}: {}", full_url, e);
                        None
                    }
                };

                if let Some(resp) = resp_opt {
                    process_and_send_response(&mut client_stream, resp, &method, &host, &path, &full_url, &req_headers, &req_body, start_time);
                }
            }
        }
    }
}
