use super::store::{MATCH_REPLACE_RULES, HEADER_INJECTION_RULES, NOISE_FILTER_SETTINGS, PASSTHROUGH_HOSTS, NoiseFilterFlags};

pub fn is_passthrough_domain(target_host: &str) -> bool {
    let host_lower = target_host.to_lowercase();
    if host_lower.contains("googlevideo.com") || host_lower.contains("gvt1.com") || host_lower.contains("ytimg.com") {
        return true;
    }
    if let Ok(lock) = PASSTHROUGH_HOSTS.lock() {
        for pattern in lock.iter() {
            let clean_pat = pattern.trim_start_matches("*.").trim_start_matches('*');
            if !clean_pat.is_empty() && host_lower.contains(clean_pat) {
                return true;
            }
        }
    }
    false
}

pub fn is_filtered_noise_request(url: &str, path: &str, headers: &str) -> bool {
    let flags = match NOISE_FILTER_SETTINGS.lock() {
        Ok(f) => f.clone(),
        Err(_) => NoiseFilterFlags::default(),
    };

    let url_lower = url.to_lowercase();
    let path_lower = path.to_lowercase();
    let headers_lower = headers.to_lowercase();

    let clean_path = path_lower.split('?').next().unwrap_or(&path_lower);
    let clean_path = clean_path.split('#').next().unwrap_or(clean_path);

    if flags.filter_scripts_styles_fonts {
        if clean_path.ends_with(".js")
            || clean_path.ends_with(".mjs")
            || clean_path.ends_with(".cjs")
            || clean_path.ends_with(".css")
            || clean_path.ends_with(".woff")
            || clean_path.ends_with(".woff2")
            || clean_path.ends_with(".ttf")
            || clean_path.ends_with(".otf")
            || clean_path.ends_with(".eot")
            || path_lower.contains(".js?")
            || path_lower.contains(".js#")
            || path_lower.contains(".css?")
            || path_lower.contains(".css#")
            || headers_lower.contains("javascript")
            || headers_lower.contains("text/css")
            || headers_lower.contains("ecmascript")
            || headers_lower.contains("font/")
        {
            return true;
        }
    }

    if flags.filter_images_media {
        if clean_path.ends_with(".png")
            || clean_path.ends_with(".jpg")
            || clean_path.ends_with(".jpeg")
            || clean_path.ends_with(".gif")
            || clean_path.ends_with(".svg")
            || clean_path.ends_with(".ico")
            || clean_path.ends_with(".webp")
            || path_lower.contains(".png?")
            || path_lower.contains(".jpg?")
            || path_lower.contains(".jpeg?")
            || path_lower.contains(".gif?")
            || path_lower.contains(".svg?")
            || path_lower.contains(".ico?")
            || headers_lower.contains("image/")
        {
            return true;
        }
    }

    if flags.filter_noisy_domains {
        if url_lower.contains("challenges.cloudflare.com")
            || url_lower.contains("google.")
            || url_lower.contains("googleapis.")
            || url_lower.contains("gstatic.")
            || url_lower.contains("googletagmanager.")
            || url_lower.contains("google-analytics.")
            || url_lower.contains("googlesyndication.")
            || url_lower.contains("googleadservices.")
            || url_lower.contains(".google")
            || url_lower.contains("yandex.")
            || url_lower.contains("yastatic.")
            || url_lower.contains("mc.yandex")
            || url_lower.contains(".yandex")
        {
            return true;
        }
    }

    false
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

/// Automatically inject custom HTTP headers into outgoing requests matching specific domains or all hosts (*).
pub fn apply_header_injection_rules(target_host: &str, mut headers: String) -> String {
    if let Ok(rules) = HEADER_INJECTION_RULES.lock() {
        for rule in rules.iter() {
            if rule.enabled && !rule.header_name.trim().is_empty() {
                let scope = rule.scope.trim();
                let matches_scope = scope == "*"
                    || scope.is_empty()
                    || target_host.to_lowercase().contains(&scope.to_lowercase());

                if matches_scope {
                    let header_line = format!("{}: {}", rule.header_name.trim(), rule.header_value.trim());
                    let lower_name = rule.header_name.trim().to_lowercase();

                    // Replace existing header or append new header line
                    let mut lines: Vec<String> = headers.lines().map(|s| s.to_string()).collect();
                    let mut found = false;
                    for line in lines.iter_mut() {
                        if line.to_lowercase().starts_with(&format!("{}:", lower_name)) {
                            *line = header_line.clone();
                            found = true;
                            break;
                        }
                    }

                    if !found {
                        lines.push(header_line);
                    }

                    headers = lines.join("\r\n");
                    if !headers.ends_with("\r\n\r\n") && !headers.ends_with("\n\n") {
                        headers.push_str("\r\n");
                    }
                }
            }
        }
    }
    headers
}
