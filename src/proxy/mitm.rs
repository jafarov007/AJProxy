use std::io::Write;
use std::net::TcpStream;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use openssl::pkey::PKey;
use openssl::ssl::{SslAcceptor, SslMethod};
use openssl::x509::X509;

use crate::proxy::cert;
use crate::proxy::filters::{apply_match_replace_rules, apply_header_injection_rules, is_filtered_noise_request, is_passthrough_domain};
use crate::proxy::http_stream::{read_full_http_request, process_and_send_response};
use crate::proxy::websocket::{is_websocket_upgrade, handle_tls_websocket_tunnel};
use crate::proxy::store::*;
use crate::proxy::listener::UPSTREAM_AGENT;

/// Full TLS MITM Interception Handler with HTTP Keep-Alive
pub fn handle_https_connect_mitm(mut client_stream: TcpStream, request_str: &str, _start_time: Instant) {
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
        let req_headers = apply_header_injection_rules(&target_host, req_headers);

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

        // ── WebSocket Upgrade Handling ────────────────────────────────────
        if is_websocket_upgrade(&req_headers) {
            if handle_tls_websocket_tunnel(tls_stream, &target_host, raw_path, &full_url, &method, &req_headers, &req_body, &req_body_bytes, request_start) {
                return;
            }
            break;
        }

        // ── PAUSE IF HTTP INTERCEPT IS ON! ─────────────────────────────────────
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
