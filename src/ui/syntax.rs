use egui::{self, FontFamily, FontId, TextFormat, text::LayoutJob};
use crate::theme::*;

pub fn http_layouter(ui: &egui::Ui, text: &str, wrap_width: f32) -> std::sync::Arc<egui::Galley> {
    let font_id = FontId::new(12.0, FontFamily::Monospace);
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_width;
    job.wrap.break_anywhere = true;

    if text.is_empty() {
        return ui.fonts(|f| f.layout_job(job));
    }

    let mut start = 0;
    let mut in_body = false;

    while start < text.len() {
        let line_end_idx = match text[start..].find('\n') {
            Some(idx) => start + idx + 1,
            None => text.len(),
        };

        let full_line = &text[start..line_end_idx];
        start = line_end_idx;

        // Separate line text from trailing \r\n or \n
        let (line_text, newline_suffix) = if full_line.ends_with("\r\n") {
            (&full_line[..full_line.len() - 2], "\r\n")
        } else if full_line.ends_with('\n') {
            (&full_line[..full_line.len() - 1], "\n")
        } else {
            (full_line, "")
        };

        if line_text.is_empty() {
            in_body = true;
            if !newline_suffix.is_empty() {
                job.append(newline_suffix, 0.0, TextFormat::simple(font_id.clone(), TEXT_2));
            }
            continue;
        }

        if !in_body {
            // Header or Request/Status Line Syntax Highlighting
            if line_text.contains(':') && !line_text.starts_with("HTTP/") && !line_text.starts_with("GET ") && !line_text.starts_with("POST ") && !line_text.starts_with("PUT ") && !line_text.starts_with("DELETE ") && !line_text.starts_with("PATCH ") && !line_text.starts_with("HEAD ") && !line_text.starts_with("OPTIONS ") {
                let parts: Vec<&str> = line_text.splitn(2, ':').collect();
                job.append(parts[0], 0.0, TextFormat {
                    font_id: font_id.clone(),
                    color: SYNTAX_KEY,
                    ..Default::default()
                });
                job.append(":", 0.0, TextFormat {
                    font_id: font_id.clone(),
                    color: TEXT_2,
                    ..Default::default()
                });
                job.append(parts[1], 0.0, TextFormat {
                    font_id: font_id.clone(),
                    color: SYNTAX_VAL,
                    ..Default::default()
                });
            } else if line_text.starts_with("GET ") || line_text.starts_with("POST ") || line_text.starts_with("PUT ") || line_text.starts_with("DELETE ") || line_text.starts_with("PATCH ") || line_text.starts_with("OPTIONS ") || line_text.starts_with("HEAD ") {
                let parts: Vec<&str> = line_text.split_whitespace().collect();
                if !parts.is_empty() {
                    let m_color = method_color(parts[0]);
                    job.append(parts[0], 0.0, TextFormat {
                        font_id: font_id.clone(),
                        color: m_color,
                        ..Default::default()
                    });
                    if parts.len() > 1 {
                        job.append(" ", 0.0, TextFormat::simple(font_id.clone(), TEXT_2));
                        job.append(parts[1], 0.0, TextFormat {
                            font_id: font_id.clone(),
                            color: TEXT_0,
                            ..Default::default()
                        });
                    }
                    if parts.len() > 2 {
                        job.append(" ", 0.0, TextFormat::simple(font_id.clone(), TEXT_2));
                        job.append(parts[2], 0.0, TextFormat {
                            font_id: font_id.clone(),
                            color: ACCENT_VIOLET,
                            ..Default::default()
                        });
                    }
                } else {
                    job.append(line_text, 0.0, TextFormat::simple(font_id.clone(), TEXT_0));
                }
            } else if line_text.starts_with("HTTP/") {
                let parts: Vec<&str> = line_text.split_whitespace().collect();
                if parts.len() >= 2 {
                    job.append(parts[0], 0.0, TextFormat {
                        font_id: font_id.clone(),
                        color: ACCENT_VIOLET,
                        ..Default::default()
                    });
                    job.append(" ", 0.0, TextFormat::simple(font_id.clone(), TEXT_2));
                    let code: u16 = parts[1].parse().unwrap_or(200);
                    job.append(parts[1], 0.0, TextFormat {
                        font_id: font_id.clone(),
                        color: status_color(code),
                        ..Default::default()
                    });
                    if parts.len() > 2 {
                        let rest = &line_text[parts[0].len() + parts[1].len() + 2..];
                        job.append(" ", 0.0, TextFormat::simple(font_id.clone(), TEXT_2));
                        job.append(rest, 0.0, TextFormat::simple(font_id.clone(), TEXT_1));
                    }
                } else {
                    job.append(line_text, 0.0, TextFormat::simple(font_id.clone(), TEXT_0));
                }
            } else {
                job.append(line_text, 0.0, TextFormat::simple(font_id.clone(), TEXT_1));
            }
        } else {
            // Body Section (JSON, URL Form-Data key=val&, GraphQL, XML Highlighting)
            highlight_body_line(&mut job, line_text, &font_id);
        }

        if !newline_suffix.is_empty() {
            job.append(newline_suffix, 0.0, TextFormat::simple(font_id.clone(), TEXT_2));
        }
    }

    ui.fonts(|f| f.layout_job(job))
}

fn highlight_body_line(job: &mut LayoutJob, line: &str, font_id: &FontId) {
    let trimmed = line.trim();

    // 1. URL Form-Data / Query Parameters: key=val&key2=val2
    if line.contains('=') && !line.contains('{') && !line.contains('<') {
        let pairs: Vec<&str> = line.split('&').collect();
        for (idx, pair) in pairs.iter().enumerate() {
            if idx > 0 {
                job.append("&", 0.0, TextFormat::simple(font_id.clone(), ACCENT_VIOLET));
            }
            if let Some((k, v)) = pair.split_once('=') {
                job.append(k, 0.0, TextFormat { font_id: font_id.clone(), color: SYNTAX_KEY, ..Default::default() });
                job.append("=", 0.0, TextFormat::simple(font_id.clone(), TEXT_2));
                job.append(v, 0.0, TextFormat { font_id: font_id.clone(), color: SYNTAX_VAL, ..Default::default() });
            } else {
                job.append(pair, 0.0, TextFormat::simple(font_id.clone(), TEXT_0));
            }
        }
        return;
    }

    // 2. GraphQL Syntax: query, mutation, fields
    if trimmed.starts_with("query ") || trimmed.starts_with("mutation ") || trimmed.starts_with("fragment ") {
        let parts: Vec<&str> = line.split_whitespace().collect();
        for (i, p) in parts.iter().enumerate() {
            if i > 0 { job.append(" ", 0.0, TextFormat::simple(font_id.clone(), TEXT_2)); }
            let color = if *p == "query" || *p == "mutation" || *p == "fragment" { ACCENT_VIOLET } else { SYNTAX_KEY };
            job.append(p, 0.0, TextFormat { font_id: font_id.clone(), color, ..Default::default() });
        }
        return;
    }

    // 3. JSON & General Structured Payload Highlighting
    if line.contains("\":") || trimmed.starts_with('"') || trimmed.starts_with('{') || trimmed.starts_with('[') {
        let mut in_string = false;
        let mut string_buf = String::new();

        for ch in line.chars() {
            if ch == '"' {
                string_buf.push(ch);
                if in_string {
                    in_string = false;
                    let color = if string_buf.contains("\":") || string_buf.ends_with("\":") {
                        SYNTAX_KEY
                    } else {
                        SYNTAX_STRING
                    };
                    job.append(&string_buf, 0.0, TextFormat {
                        font_id: font_id.clone(),
                        color,
                        ..Default::default()
                    });
                    string_buf.clear();
                } else {
                    in_string = true;
                }
            } else if in_string {
                string_buf.push(ch);
            } else {
                let char_str = ch.to_string();
                let color = match ch {
                    '{' | '}' | '[' | ']' | ',' | ':' => TEXT_2,
                    '0'..='9' => SYNTAX_NUMBER,
                    _ => TEXT_0,
                };
                job.append(&char_str, 0.0, TextFormat {
                    font_id: font_id.clone(),
                    color,
                    ..Default::default()
                });
            }
        }
        if !string_buf.is_empty() {
            job.append(&string_buf, 0.0, TextFormat {
                font_id: font_id.clone(),
                color: SYNTAX_STRING,
                ..Default::default()
            });
        }
        return;
    }

    // 4. Default Line
    job.append(line, 0.0, TextFormat::simple(font_id.clone(), TEXT_1));
}
