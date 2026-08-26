use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;
use openssl::x509::X509;
use openssl::pkey::PKey;
use openssl::ssl::{SslMethod, SslAcceptor};

use crate::models::{HttpEntry, InterceptRule};
use crate::proxy::cert;

static NEXT_ID: AtomicU32 = AtomicU32::new(1);
static INTERCEPT_ENABLED: AtomicBool = AtomicBool::new(false); // DEFAULT OFF!

pub fn set_intercept_enabled(enabled: bool) {
    INTERCEPT_ENABLED.store(enabled, Ordering::SeqCst);
}

pub fn is_intercept_enabled() -> bool {
    INTERCEPT_ENABLED.load(Ordering::SeqCst)
}

pub enum InterceptDecision {
    Forward,
    Drop,
}

#[derive(Clone)]
pub struct PendingIntercept {
    pub id: u32,
    pub method: String,
    pub host: String,
    pub path: String,
    pub url: String,
    pub headers: String,
    pub body: String,
    pub responder: Arc<Mutex<Option<Sender<InterceptDecision>>>>,
}

lazy_static::lazy_static! {
    pub static ref TRAFFIC_STORE: Arc<Mutex<Vec<HttpEntry>>> = Arc::new(Mutex::new(Vec::new()));
    pub static ref PENDING_INTERCEPTS: Arc<Mutex<Vec<PendingIntercept>>> = Arc::new(Mutex::new(Vec::new()));
    pub static ref MATCH_REPLACE_RULES: Arc<Mutex<Vec<InterceptRule>>> = Arc::new(Mutex::new(Vec::new()));
}

pub fn update_match_rules(rules: Vec<InterceptRule>) {
    if let Ok(mut lock) = MATCH_REPLACE_RULES.lock() {
        *lock = rules;
    }
}

pub fn apply_match_replace_rules(mut headers: String, mut body: String) -> (String, String) {
    if let Ok(rules) = MATCH_REPLACE_RULES.lock() {
        for rule in rules.iter() {
            if rule.enabled && !rule.pattern.is_empty() {
                match rule.match_type.as_str() {
                    "Header" => {
                        headers = headers.replace(&rule.pattern, &rule.action);
                    }
                    "Request Body" => {
                        body = body.replace(&rule.pattern, &rule.action);
                    }
                    "URL / Path" => {
                        headers = headers.replace(&rule.pattern, &rule.action);
                    }
                    _ => {
                        headers = headers.replace(&rule.pattern, &rule.action);
                        body = body.replace(&rule.pattern, &rule.action);
                    }
                }
            }
        }
    }
    (headers, body)
}

pub fn push_captured_entry(entry: HttpEntry) {
    if let Ok(mut store) = TRAFFIC_STORE.lock() {
        store.push(entry);
    }
}

pub fn get_captured_entries() -> Vec<HttpEntry> {
    if let Ok(store) = TRAFFIC_STORE.lock() {
        store.clone()
    } else {
        Vec::new()
    }
}

pub fn get_pending_intercepts() -> Vec<PendingIntercept> {
    if let Ok(lock) = PENDING_INTERCEPTS.lock() {
        lock.clone()
    } else {
        Vec::new()
    }
}

pub fn resolve_pending_intercept(id: u32, decision: InterceptDecision) {
    let mut sender_opt = None;
    if let Ok(mut lock) = PENDING_INTERCEPTS.lock() {
        if let Some(pos) = lock.iter().position(|p| p.id == id) {
            let item = lock.remove(pos);
            let responder = item.responder.clone();
            if let Ok(mut s_lock) = responder.lock() {
                sender_opt = s_lock.take();
            };
        }
    }
    if let Some(sender) = sender_opt {
        let _ = sender.send(decision);
    }
}

#[allow(dead_code)]
pub fn clear_traffic_store() {
    if let Ok(mut store) = TRAFFIC_STORE.lock() {
        store.clear();
    }
}

/// Helper function to split raw HTTP bytes into (headers, body)
fn parse_raw_http(raw_bytes: &[u8]) -> (String, String) {
    if let Some(pos) = raw_bytes.windows(4).position(|w| w == b"\r\n\r\n") {
        let headers = String::from_utf8_lossy(&raw_bytes[..pos]).to_string();
        let body = String::from_utf8_lossy(&raw_bytes[pos + 4..]).to_string();
        (headers, body)
    } else {
        (String::from_utf8_lossy(raw_bytes).to_string(), String::new())
    }
}

/// Starts a multithreaded TCP HTTP/HTTPS Proxy Listener on 127.0.0.1:<port>
pub fn start_proxy_server(bind_addr: String, bind_port: u16) {
    let address = format!("{}:{}", bind_addr, bind_port);

    thread::spawn(move || {
        let listener = match TcpListener::bind(&address) {
            Ok(l) => {
                println!("[AJProxy Listener] Successfully bound & listening on {}", address);
                l
            }
            Err(e) => {
                eprintln!("[AJProxy Listener] Error binding to {}: {}", address, e);
                return;
            }
        };

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    thread::spawn(move || {
                        handle_client_connection(stream);
                    });
                }
                Err(e) => {
                    eprintln!("[AJProxy Listener] Connection error: {}", e);
                }
            }
        }
    });
}

fn handle_client_connection(mut client_stream: TcpStream) {
    let start_time = Instant::now();
    let mut buffer = [0u8; 65536];
    let bytes_read = match client_stream.read(&mut buffer) {
        Ok(n) if n > 0 => n,
        _ => return,
    };

    let raw_request = &buffer[..bytes_read];
    let (req_headers, req_body) = parse_raw_http(raw_request);
    let (req_headers, req_body) = apply_match_replace_rules(req_headers, req_body);
    let request_str = &req_headers;

    // Handle HTTPS CONNECT tunnel request with TLS MITM Decryption
    if request_str.starts_with("CONNECT ") {
        handle_https_connect_mitm(client_stream, request_str, start_time);
        return;
    }

    // Handle Direct Proxy Welcome / Status Page
    if request_str.starts_with("GET / ") && (request_str.contains("Host: 127.0.0.1") || request_str.contains("Host: localhost")) {
        let response_body = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>AJProxy — Interceptor Active</title>
    <style>
        body { font-family: system-ui, sans-serif; background: #121214; color: #60a5fa; text-align: center; padding-top: 100px; }
        .card { background: #161820; border: 1px solid #025096; padding: 40px; display: inline-block; border-radius: 12px; max-width: 600px; }
        h1 { color: #4ade80; margin-bottom: 10px; }
        p { color: #94a3b8; font-size: 14px; }
        .badge { background: #063464; color: #38bdf8; padding: 6px 12px; border-radius: 6px; font-family: monospace; }
    </style>
</head>
<body>
    <div class="card">
        <h1>✔ AJProxy Interceptor Active</h1>
        <p>Your browser is successfully proxied through AJProxy on <span class="badge">127.0.0.1:8080</span>.</p>
        <p>All HTTP/HTTPS requests are being recorded and intercepted in real-time.</p>
    </div>
</body>
</html>"#;

        let http_response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );

        let _ = client_stream.write_all(http_response.as_bytes());

        push_captured_entry(HttpEntry {
            id: NEXT_ID.fetch_add(1, Ordering::SeqCst),
            timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
            method: "GET".to_string(),
            host: "127.0.0.1:8080".to_string(),
            path: "/".to_string(),
            url: "http://127.0.0.1:8080/".to_string(),
            status_code: 200,
            content_type: "text/html".to_string(),
            length: response_body.len(),
            duration_ms: start_time.elapsed().as_millis() as u64,
            protocol: "HTTP/1.1".to_string(),
            request_headers: req_headers,
            request_body: req_body,
            response_headers: format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}", response_body.len()),
            response_body: response_body.to_string(),
        });
        return;
    }

    // Handle Plain HTTP Proxy Forwarding
    forward_http_request(client_stream, &req_headers, &req_body, start_time);
}

/// Helper function to process response and forward to client
fn process_and_send_response(
    tls_stream: &mut openssl::ssl::SslStream<TcpStream>,
    resp: ureq::Response,
    method: &str,
    target_host: &str,
    raw_path: &str,
    full_url: &str,
    req_headers: &str,
    req_body: &str,
    start_time: Instant,
) {
    let status = resp.status();
    let content_type = resp.header("Content-Type").unwrap_or("text/html").to_string();

    let mut resp_headers_str = format!("HTTP/1.1 {} OK\r\n", status);
    for h_name in resp.headers_names() {
        if let Some(h_val) = resp.header(&h_name) {
            resp_headers_str.push_str(&format!("{}: {}\r\n", h_name, h_val));
        }
    }

    let mut body_bytes = Vec::new();
    let _ = resp.into_reader().read_to_end(&mut body_bytes);

    let http_resp = format!(
        "HTTP/1.1 {} OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        status, content_type, body_bytes.len()
    );

    let _ = tls_stream.write_all(http_resp.as_bytes());
    let _ = tls_stream.write_all(&body_bytes);
    let _ = tls_stream.flush();

    let parsed_url = url::Url::parse(full_url).ok();
    let host = parsed_url.as_ref().and_then(|u| u.host_str()).unwrap_or(target_host).to_string();
    let path = parsed_url.as_ref().map(|u| u.path()).unwrap_or(raw_path).to_string();

    push_captured_entry(HttpEntry {
        id: NEXT_ID.fetch_add(1, Ordering::SeqCst),
        timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
        method: method.to_string(),
        host,
        path,
        url: full_url.to_string(),
        status_code: status,
        content_type: content_type.clone(),
        length: body_bytes.len(),
        duration_ms: start_time.elapsed().as_millis() as u64,
        protocol: "HTTP/1.1".to_string(),
        request_headers: req_headers.to_string(),
        request_body: req_body.to_string(),
        response_headers: resp_headers_str,
        response_body: String::from_utf8_lossy(&body_bytes[..body_bytes.len().min(16384)]).to_string(),
    });
}

/// Full TLS MITM Interception Handler
fn handle_https_connect_mitm(mut client_stream: TcpStream, request_str: &str, start_time: Instant) {
    let first_line = request_str.lines().next().unwrap_or("");
    let target = first_line
        .strip_prefix("CONNECT ")
        .and_then(|s| s.split_whitespace().next())
        .unwrap_or("");

    if target.is_empty() {
        return;
    }

    let target_host = target.split(':').next().unwrap_or(target).to_string();

    // Send 200 Connection Established ACK
    let ack = "HTTP/1.1 200 Connection Established\r\nProxy-Agent: AJProxy/0.1\r\n\r\n";
    if client_stream.write_all(ack.as_bytes()).is_err() {
        return;
    }

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
            eprintln!("[AJProxy MITM] Handshake failed with browser for {}: {}", target_host, e);
            return;
        }
    };

    let mut buf = [0u8; 65536];
    let n = match tls_stream.read(&mut buf) {
        Ok(n) if n > 0 => n,
        _ => return,
    };

    let (req_headers, req_body) = parse_raw_http(&buf[..n]);
    let (req_headers, req_body) = apply_match_replace_rules(req_headers, req_body);

    let first_line = req_headers.lines().next().unwrap_or("");
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }

    let method = parts[0].to_uppercase();
    let raw_path = parts[1];

    let full_url = if raw_path.starts_with("http://") || raw_path.starts_with("https://") {
        raw_path.to_string()
    } else {
        format!("https://{}{}", target_host, raw_path)
    };

    // ── PAUSE IF INTERCEPT IS ON! ─────────────────────────────────────────
    if is_intercept_enabled() {
        let (tx, rx) = channel();
        let entry_id = NEXT_ID.fetch_add(1, Ordering::SeqCst);

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

        // Wait for user action in Intercept UI tab (Forward vs Drop)
        match rx.recv() {
            Ok(InterceptDecision::Forward) => {
                // User clicked Forward! Proceed to real target server!
            }
            _ => {
                // User clicked Drop! Return 502 Bad Gateway to browser!
                let drop_resp = "HTTP/1.1 502 Bad Gateway\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nRequest dropped by AJProxy Interceptor.";
                let _ = tls_stream.write_all(drop_resp.as_bytes());
                return;
            }
        }
    }

    // Forward decrypted request to real target server via HTTPS
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(12))
        .build();

    let mut req = agent.request(&method, &full_url);

    for line in req_headers.lines().skip(1) {
        if let Some((k, v)) = line.split_once(':') {
            let k = k.trim();
            let v = v.trim();
            if !k.eq_ignore_ascii_case("Proxy-Connection") && !k.eq_ignore_ascii_case("Host") && !k.eq_ignore_ascii_case("Accept-Encoding") {
                req = req.set(k, v);
            }
        }
    }

    let send_res = if !req_body.is_empty() {
        req.send_string(&req_body)
    } else {
        req.call()
    };

    match send_res {
        Ok(resp) => {
            process_and_send_response(&mut tls_stream, resp, &method, &target_host, raw_path, &full_url, &req_headers, &req_body, start_time);
        }
        Err(ureq::Error::Status(_, resp)) => {
            process_and_send_response(&mut tls_stream, resp, &method, &target_host, raw_path, &full_url, &req_headers, &req_body, start_time);
        }
        Err(e) => {
            eprintln!("[AJProxy MITM] Upstream HTTPS request failed for {}: {}", full_url, e);
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
                if is_intercept_enabled() {
                    let (tx, rx) = channel();
                    let entry_id = NEXT_ID.fetch_add(1, Ordering::SeqCst);

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

                let agent = ureq::AgentBuilder::new()
                    .timeout(std::time::Duration::from_secs(12))
                    .build();

                let mut req = agent.request(&method, &full_url);

                for line in req_headers.lines().skip(1) {
                    if let Some((k, v)) = line.split_once(':') {
                        let k = k.trim();
                        let v = v.trim();
                        if !k.eq_ignore_ascii_case("Proxy-Connection") && !k.eq_ignore_ascii_case("Host") && !k.eq_ignore_ascii_case("Accept-Encoding") {
                            req = req.set(k, v);
                        }
                    }
                }

                let send_res = if !req_body.is_empty() {
                    req.send_string(&req_body)
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
                    let status = resp.status();
                    let content_type = resp.header("Content-Type").unwrap_or("text/html").to_string();

                    let mut resp_headers_str = format!("HTTP/1.1 {} OK\r\n", status);
                    for h_name in resp.headers_names() {
                        if let Some(h_val) = resp.header(&h_name) {
                            resp_headers_str.push_str(&format!("{}: {}\r\n", h_name, h_val));
                        }
                    }

                    let mut body_bytes = Vec::new();
                    let _ = resp.into_reader().read_to_end(&mut body_bytes);

                    let http_resp = format!(
                        "HTTP/1.1 {} OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        status, content_type, body_bytes.len()
                    );

                    let _ = client_stream.write_all(http_resp.as_bytes());
                    let _ = client_stream.write_all(&body_bytes);

                    push_captured_entry(HttpEntry {
                        id: NEXT_ID.fetch_add(1, Ordering::SeqCst),
                        timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                        method: method.to_string(),
                        host,
                        path,
                        url: full_url,
                        status_code: status,
                        content_type: content_type.clone(),
                        length: body_bytes.len(),
                        duration_ms: start_time.elapsed().as_millis() as u64,
                        protocol: "HTTP/1.1".to_string(),
                        request_headers: req_headers.to_string(),
                        request_body: req_body.to_string(),
                        response_headers: resp_headers_str,
                        response_body: String::from_utf8_lossy(&body_bytes[..body_bytes.len().min(16384)]).to_string(),
                    });
                }
            }
        }
    }
}
