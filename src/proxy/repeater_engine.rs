use std::io::Read;
use std::time::Instant;
use crate::models::{RepeaterTab, RepeaterStatus};

/// Executes an outbound HTTP/HTTPS request for a Repeater tab using ureq.
pub fn execute_repeater_request(tab: &mut RepeaterTab) {
    let start_time = Instant::now();
    tab.status = RepeaterStatus::Sending;

    let request_str = &tab.request_text;
    let (req_headers, req_body) = if let Some(pos) = request_str.find("\r\n\r\n").or_else(|| request_str.find("\n\n")) {
        let sep_len = if request_str.contains("\r\n\r\n") { 4 } else { 2 };
        (&request_str[..pos], &request_str[pos + sep_len..])
    } else {
        (request_str.as_str(), "")
    };

    let first_line = req_headers.lines().next().unwrap_or("GET / HTTP/1.1");
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    let method = parts.get(0).cloned().unwrap_or("GET").to_uppercase();
    let path = parts.get(1).cloned().unwrap_or("/");

    let scheme = if tab.is_tls { "https" } else { "http" };
    let host = tab.target_host.trim();
    let port = tab.target_port.trim();

    let target_url = if (scheme == "https" && (port == "443" || port.is_empty())) || (scheme == "http" && (port == "80" || port.is_empty())) {
        format!("{}://{}{}", scheme, host, path)
    } else {
        format!("{}://{}:{}{}", scheme, host, port, path)
    };

    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(15))
        .build();

    let mut req = agent.request(&method, &target_url);

    for line in req_headers.lines().skip(1) {
        if let Some((k, v)) = line.split_once(':') {
            let k = k.trim();
            let v = v.trim();
            if !k.eq_ignore_ascii_case("Host") && !k.eq_ignore_ascii_case("Accept-Encoding") {
                req = req.set(k, v);
            }
        }
    }

    let send_res = if !req_body.is_empty() {
        req.send_string(req_body)
    } else {
        req.call()
    };

    let elapsed = start_time.elapsed().as_millis() as u64;
    tab.response_time_ms = elapsed;
    tab.status = RepeaterStatus::Done;

    let resp_opt = match send_res {
        Ok(r) => Some(r),
        Err(ureq::Error::Status(_, r)) => Some(r),
        Err(e) => {
            tab.response_text = format!("HTTP/1.1 500 Connection Error\r\nContent-Type: text/plain\r\n\r\n[AJProxy Repeater Error]: {}", e);
            None
        }
    };

    if let Some(resp) = resp_opt {
        let status = resp.status();
        let mut raw_resp = format!("HTTP/1.1 {} OK\r\n", status);
        for h_name in resp.headers_names() {
            if let Some(h_val) = resp.header(&h_name) {
                raw_resp.push_str(&format!("{}: {}\r\n", h_name, h_val));
            }
        }
        raw_resp.push_str("\r\n");

        let mut body_bytes = Vec::new();
        let _ = resp.into_reader().read_to_end(&mut body_bytes);
        raw_resp.push_str(&String::from_utf8_lossy(&body_bytes));

        tab.response_text = raw_resp;
    }
}
