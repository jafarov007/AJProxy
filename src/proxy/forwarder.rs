use std::io::Write;
use std::net::TcpStream;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::proxy::filters::{apply_match_replace_rules, is_filtered_noise_request};
use crate::proxy::http_stream::process_and_send_response;
use crate::proxy::websocket::{is_websocket_upgrade, handle_plain_websocket_tunnel};
use crate::proxy::store::*;
use crate::proxy::listener::UPSTREAM_AGENT;

pub fn forward_http_request(mut client_stream: TcpStream, req_headers: &str, req_body: &str, start_time: Instant) {
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
