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
    pub static ref UPSTREAM_AGENT: ureq::Agent = ureq::AgentBuilder::new()
        .redirects(0)
        .timeout(std::time::Duration::from_secs(30))
        .max_idle_connections(200)
        .max_idle_connections_per_host(20)
        .build();
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

/// Helper function to read a full HTTP request (headers + body) from a stream.
/// It reads the headers first, parses the Content-Length, and then reads the exact remaining body bytes.
fn read_full_http_request<R: std::io::Read>(reader: &mut R) -> Result<(String, Vec<u8>), std::io::Error> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    let mut header_end_pos = None;

    // 1. Read until we find "\r\n\r\n"
    loop {
        let n = reader.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..n]);
        if let Some(pos) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
            header_end_pos = Some(pos);
            break;
        }
    }

    let pos = match header_end_pos {
        Some(p) => p,
        None => {
            let headers = String::from_utf8_lossy(&buffer).to_string();
            return Ok((headers, Vec::new()));
        }
    };

    let headers_str = String::from_utf8_lossy(&buffer[..pos]).to_string();
    let mut body_bytes = buffer[pos + 4..].to_vec();

    // 2. Parse Content-Length
    let mut content_length = 0;
    for line in headers_str.lines() {
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().eq_ignore_ascii_case("content-length") {
                if let Ok(len) = v.trim().parse::<usize>() {
                    content_length = len;
                }
            }
        }
    }

    // 3. Read the rest of the body if needed
    if body_bytes.len() < content_length {
        let mut remaining = content_length - body_bytes.len();
        let mut body_chunk = vec![0u8; remaining.min(4096)];
        while remaining > 0 {
            let n = reader.read(&mut body_chunk)?;
            if n == 0 {
                break;
            }
            body_bytes.extend_from_slice(&body_chunk[..n]);
            remaining -= n;
            if remaining > 0 && body_chunk.len() > remaining {
                body_chunk.resize(remaining, 0);
            }
        }
    }

    Ok((headers_str, body_bytes))
}

/// Helper function to split raw HTTP bytes into (headers, body)
#[allow(dead_code)]
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
    let _ = client_stream.set_read_timeout(Some(std::time::Duration::from_secs(15)));
    let _ = client_stream.set_write_timeout(Some(std::time::Duration::from_secs(15)));
    let (req_headers, req_body_bytes) = match read_full_http_request(&mut client_stream) {
        Ok(res) => res,
        Err(_) => return,
    };
    let req_body = String::from_utf8_lossy(&req_body_bytes).to_string();
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

    let status_str = match status {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        206 => "Partial Content",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        304 => "Not Modified",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "OK",
    };

    // Collect all response headers for logging
    let mut resp_headers_str = format!("HTTP/1.1 {} {}\r\n", status, status_str);
    // Build actual response headers to send to browser
    // Key: strip Content-Length, Transfer-Encoding, Content-Encoding
    // because ureq auto-decompresses, so original values are wrong.
    // We set our own correct Content-Length after reading the full body.
    let mut forwarded_headers = String::new();
    for h_name in resp.headers_names() {
        if let Some(h_val) = resp.header(&h_name) {
            resp_headers_str.push_str(&format!("{}: {}\r\n", h_name, h_val));
            if !h_name.eq_ignore_ascii_case("content-length")
                && !h_name.eq_ignore_ascii_case("transfer-encoding")
                && !h_name.eq_ignore_ascii_case("content-encoding")
            {
                forwarded_headers.push_str(&format!("{}: {}\r\n", h_name, h_val));
            }
        }
    }

    // Read full (already decompressed by ureq) response body
    let mut body_bytes = Vec::new();
    let _ = resp.into_reader().read_to_end(&mut body_bytes);

    // Build final HTTP response with correct Content-Length
    let mut http_resp = format!("HTTP/1.1 {} {}\r\n", status, status_str);
    http_resp.push_str(&forwarded_headers);
    http_resp.push_str(&format!("Content-Length: {}\r\n", body_bytes.len()));
    http_resp.push_str("\r\n");

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

    // ── Direct TCP Passthrough for Video streaming CDNs & heavy media ──
    let is_passthrough = target_host.contains("googlevideo.com")
        || target_host.contains("gvt1.com")
        || target_host.contains("ytimg.com");

    if is_passthrough {
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

            match rx.recv() {
                Ok(InterceptDecision::Forward) => {}
                _ => {
                    let drop_resp = "HTTP/1.1 502 Bad Gateway\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nRequest dropped by AJProxy Interceptor.";
                    let _ = tls_stream.write_all(drop_resp.as_bytes());
                    break;
                }
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
                // Suppress spam for known-noisy Chrome background services, DNS failures & ad trackers
                let is_noisy = full_url.contains("android.clients.google.com/checkin")
                    || full_url.contains("clients1.google.com")
                    || full_url.contains("clients2.google.com")
                    || full_url.contains("update.googleapis.com")
                    || full_url.contains("localhost.sensic.net")
                    || full_url.contains("omnitagjs.com")
                    || full_url.contains("presage.io")
                    || full_url.contains("rubiconproject.com")
                    || full_url.contains("play.google.com/log")
                    || full_url.contains("cspreport")
                    || full_url.contains("spotxchange.com")
                    || full_url.contains("bluekai.com")
                    || full_url.contains("addthis.com")
                    || full_url.contains("lkqd.net")
                    || full_url.contains("iqzone.com")
                    || full_url.contains("colossusssp.com")
                    || full_url.contains("stickyadstv.com")
                    || full_url.contains("yieldmo.com")
                    || full_url.contains("mathtag.com")
                    || full_url.contains("drift-pixel.ai")
                    || full_url.contains("gammaplatform.com")
                    || full_url.contains("yandex.net")
                    || full_url.contains("amitydigital.io")
                    || full_url.contains("rtb-oveeo.com")
                    || full_url.contains("googlevideo.com")
                    || err_str.contains("HTTP version")
                    || err_str.contains("Name or service not known")
                    || err_str.contains("Connection refused");
                if !is_noisy {
                    eprintln!("[AJProxy MITM] Upstream error: {} → {}", full_url, e);
                }
                // Send 502 to browser and keep the loop alive
                let error_body = "502 Bad Gateway";
                let error_resp = format!(
                    "HTTP/1.1 502 Bad Gateway\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                    error_body.len(), error_body
                );
                if tls_stream.write_all(error_resp.as_bytes()).is_err() {
                    break; // Browser connection dead
                }
                let _ = tls_stream.flush();
                continue; // Keep loop alive for next request
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
                    let status = resp.status();
                    let content_type = resp.header("Content-Type").unwrap_or("text/html").to_string();

                    let status_str = match status {
                        200 => "OK",
                        201 => "Created",
                        202 => "Accepted",
                        204 => "No Content",
                        206 => "Partial Content",
                        301 => "Moved Permanently",
                        302 => "Found",
                        303 => "See Other",
                        304 => "Not Modified",
                        307 => "Temporary Redirect",
                        308 => "Permanent Redirect",
                        400 => "Bad Request",
                        401 => "Unauthorized",
                        403 => "Forbidden",
                        404 => "Not Found",
                        405 => "Method Not Allowed",
                        409 => "Conflict",
                        500 => "Internal Server Error",
                        502 => "Bad Gateway",
                        503 => "Service Unavailable",
                        504 => "Gateway Timeout",
                        _ => "OK",
                    };

                    // Collect all response headers for logging
                    let mut resp_headers_str = format!("HTTP/1.1 {} {}\r\n", status, status_str);
                    let mut forwarded_headers = String::new();
                    for h_name in resp.headers_names() {
                        if let Some(h_val) = resp.header(&h_name) {
                            resp_headers_str.push_str(&format!("{}: {}\r\n", h_name, h_val));
                            if !h_name.eq_ignore_ascii_case("content-length")
                                && !h_name.eq_ignore_ascii_case("transfer-encoding")
                                && !h_name.eq_ignore_ascii_case("content-encoding")
                            {
                                forwarded_headers.push_str(&format!("{}: {}\r\n", h_name, h_val));
                            }
                        }
                    }

                    let mut body_bytes = Vec::new();
                    let _ = resp.into_reader().read_to_end(&mut body_bytes);

                    let mut http_resp = format!("HTTP/1.1 {} {}\r\n", status, status_str);
                    http_resp.push_str(&forwarded_headers);
                    http_resp.push_str(&format!("Content-Length: {}\r\n", body_bytes.len()));
                    http_resp.push_str("Connection: close\r\n\r\n");

                    let _ = client_stream.write_all(http_resp.as_bytes());
                    let _ = client_stream.write_all(&body_bytes);

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
            }
        }
    }
}
