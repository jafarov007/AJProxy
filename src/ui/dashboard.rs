use egui::{self, RichText, ScrollArea, FontFamily};
use std::collections::HashSet;
use crate::models::*;
use crate::theme::*;

pub fn render(ui: &mut egui::Ui, entries: &[HttpEntry], proxy_running: bool) {
    let total_requests = entries.len();
    let unique_hosts: HashSet<_> = entries.iter().map(|e| &e.host).collect();
    let total_bytes: usize = entries.iter().map(|e| e.length).sum();

    // ── Stats row ─────────────────────────────────────────────────
    ui.columns(4, |cols| {
        let stats = [
            ("Captured Packets", format!("{}", total_requests), TEXT_0),
            ("Proxy Listener", if proxy_running { "Active" } else { "Stopped" }.to_string(), if proxy_running { ACCENT_GREEN } else { ACCENT_RED }),
            ("Unique Hosts", format!("{}", unique_hosts.len()), TEXT_1),
            ("Data Transferred", format!("{:.2} KB", total_bytes as f64 / 1024.0), TEXT_1),
        ];

        for (i, (label, value, color)) in stats.iter().enumerate() {
            section_frame().show(&mut cols[i], |ui| {
                ui.label(RichText::new(*label).size(10.0).color(TEXT_2));
                ui.label(RichText::new(value).size(16.0).color(*color).strong().family(FontFamily::Monospace));
            });
        }
    });

    ui.add_space(6.0);

    // ── Two-column layout ───────────────────────────
    ui.columns(2, |cols| {
        // Left: Recent Live Requests
        section_frame().show(&mut cols[0], |ui| {
            ui.label(RichText::new("Live Traffic Stream").size(11.0).color(TEXT_0).strong());
            ui.separator();

            if entries.is_empty() {
                ui.add_space(30.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("🌐 Waiting for live traffic...").size(13.0).color(TEXT_2));
                    ui.label(RichText::new("Open 🌐 Browser to start intercepting real packets").size(11.0).color(TEXT_2));
                });
            } else {
                ScrollArea::vertical()
                    .id_source("dashboard_recent_scroll")
                    .max_height(400.0)
                    .show(ui, |ui| {
                        for entry in entries.iter().rev().take(15) {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&entry.timestamp).size(10.0).color(TEXT_2).family(FontFamily::Monospace));
                                ui.label(RichText::new(&entry.method).size(10.0).color(method_color(&entry.method)).strong().family(FontFamily::Monospace));
                                ui.label(RichText::new(&entry.host).size(10.0).color(TEXT_0));
                                ui.label(RichText::new(&entry.path).size(10.0).color(TEXT_1).family(FontFamily::Monospace));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(RichText::new(format!("{}", entry.status_code)).size(10.0).color(status_color(entry.status_code)).strong().family(FontFamily::Monospace));
                                });
                            });
                        }
                    });
            }
        });

        // Right: Engine Info & Real Status
        section_frame().show(&mut cols[1], |ui| {
            ui.label(RichText::new("Proxy Engine Status").size(11.0).color(TEXT_0).strong());
            ui.separator();

            let rows = [
                ("Listener Socket", "127.0.0.1:8080"),
                ("Intercept State", if proxy_running { "Running & Capturing" } else { "Paused" }),
                ("TLS MITM", "Enabled (ca_cert.pem)"),
                ("Storage", "Real-Time In-Memory Buffer"),
            ];
            for (k, v) in &rows {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(*k).size(11.0).color(TEXT_2));
                    ui.label(RichText::new(*v).size(11.0).color(TEXT_0).family(FontFamily::Monospace));
                });
            }

            ui.add_space(16.0);
            ui.label(RichText::new("HTTP Status Distribution").size(11.0).color(TEXT_2));
            ui.separator();

            let count_2xx = entries.iter().filter(|e| e.status_code >= 200 && e.status_code < 300).count();
            let count_3xx = entries.iter().filter(|e| e.status_code >= 300 && e.status_code < 400).count();
            let count_4xx_5xx = entries.iter().filter(|e| e.status_code >= 400).count();

            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("2xx OK: {}", count_2xx)).size(11.0).color(ACCENT_GREEN).family(FontFamily::Monospace));
                ui.add_space(12.0);
                ui.label(RichText::new(format!("3xx Redir: {}", count_3xx)).size(11.0).color(ACCENT_BLUE).family(FontFamily::Monospace));
                ui.add_space(12.0);
                ui.label(RichText::new(format!("4xx/5xx Err: {}", count_4xx_5xx)).size(11.0).color(ACCENT_RED).family(FontFamily::Monospace));
            });
        });
    });
}
