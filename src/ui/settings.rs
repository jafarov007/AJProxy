use egui::{self, RichText, Rounding, Stroke, ScrollArea, FontFamily};
use crate::models::*;
use crate::theme::*;
use crate::proxy::cert;

pub fn render(ui: &mut egui::Ui, settings: &mut AppSettings) {
    ScrollArea::vertical()
        .id_source("settings_scroll_area")
        .show(ui, |ui| {
            // ── Section 1: Proxy Listeners & Binding Settings ─────────────────
            section_frame().show(ui, |ui| {
                let mut listener_changed = false;

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Proxy Listeners & Binding Interfaces").size(13.0).color(TEXT_0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(
                            egui::Button::new(RichText::new("+ Add New Listener").size(11.0).color(TEXT_0).strong())
                                .fill(ACCENT_BLUE)
                                .rounding(Rounding::same(4.0))
                        ).clicked() {
                            let next_port = 8080 + settings.listeners.len() as u16;
                            settings.listeners.push(ProxyListenerConfig {
                                enabled: true,
                                bind_address: "0.0.0.0".into(),
                                bind_port: next_port,
                                protocol: "Auto".into(),
                                tls_mitm: true,
                            });
                            listener_changed = true;
                        }
                    });
                });
                ui.label(RichText::new("Configure binding addresses and ports (Use 0.0.0.0 for mobile device proxying).").size(11.0).color(TEXT_2));
                ui.separator();
                ui.add_space(4.0);

                // Listeners Grid Table
                let mut to_delete: Option<usize> = None;

                // Table Header
                ui.horizontal(|ui| {
                    ui.add_space(30.0);
                    ui.label(RichText::new("Bind Address").size(11.0).color(TEXT_1).strong());
                    ui.add_space(60.0);
                    ui.label(RichText::new("Port").size(11.0).color(TEXT_1).strong());
                    ui.add_space(30.0);
                    ui.label(RichText::new("Status").size(11.0).color(TEXT_1).strong());
                    ui.add_space(35.0);
                    ui.label(RichText::new("Protocol").size(11.0).color(TEXT_1).strong());
                    ui.add_space(20.0);
                    ui.label(RichText::new("TLS MITM").size(11.0).color(TEXT_1).strong());
                });
                ui.separator();

                for (idx, listener) in settings.listeners.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        if ui.checkbox(&mut listener.enabled, "").changed() {
                            listener_changed = true;
                        }

                        // Address Input
                        if ui.add(egui::TextEdit::singleline(&mut listener.bind_address).desired_width(130.0)).changed() {
                            listener_changed = true;
                        }

                        // Port Input
                        let mut p_str = listener.bind_port.to_string();
                        if ui.add(egui::TextEdit::singleline(&mut p_str).desired_width(55.0)).changed() {
                            if let Ok(p) = p_str.parse::<u16>() {
                                listener.bind_port = p;
                                listener_changed = true;
                            }
                        }

                        // Status Badge
                        let is_running = crate::proxy::listener::is_listener_running(&listener.bind_address, listener.bind_port);
                        let (status_text, status_color) = if is_running && listener.enabled {
                            ("● Running", ACCENT_GREEN)
                        } else {
                            ("○ Stopped", TEXT_2)
                        };
                        ui.label(RichText::new(status_text).size(10.0).color(status_color).strong());

                        // Protocol Selector
                        egui::ComboBox::from_id_source(format!("proto_combo_{}", idx))
                            .selected_text(&listener.protocol)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut listener.protocol, "Auto".into(), "Auto");
                                ui.selectable_value(&mut listener.protocol, "HTTP/1.1".into(), "HTTP/1.1");
                                ui.selectable_value(&mut listener.protocol, "HTTP/2".into(), "HTTP/2");
                            });

                        // TLS Checkbox
                        ui.checkbox(&mut listener.tls_mitm, "Enable TLS");

                        // Delete Listener Button
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(
                                egui::Button::new(RichText::new("✖ Delete").size(11.0).color(ACCENT_RED))
                                    .fill(BG_RAISED)
                                    .stroke(Stroke::new(0.5_f32, BORDER))
                            ).clicked() {
                                to_delete = Some(idx);
                            }
                        });
                    });
                    ui.add_space(2.0);
                }

                if let Some(idx) = to_delete {
                    if settings.listeners.len() > 1 {
                        settings.listeners.remove(idx);
                        listener_changed = true;
                    } else {
                        settings.cert_status_msg = "Cannot delete the last remaining listener.".into();
                    }
                }

                // Keep main listen_address and listen_port in sync & trigger dynamic socket binding on change
                settings.sync_active_listener();
                if listener_changed {
                    crate::proxy::listener::sync_listeners(&settings.listeners);
                }
            });

            ui.add_space(8.0);

            // ── Section 2: CA Certificate Management (3 Action Buttons + File Dialogs) ─────
            section_frame().show(ui, |ui| {
                ui.label(RichText::new("CA Certificate Management").size(13.0).color(TEXT_0).strong());
                ui.label(RichText::new("Manage Root CA Certificate for HTTPS MITM inspection.").size(11.0).color(TEXT_2));
                ui.separator();
                ui.add_space(4.0);

                // Current Active CA Location
                let ca_path = cert::get_cert_path();
                let ca_exists = ca_path.exists();
                let status_icon = if ca_exists { "● Active" } else { "○ Missing" };
                let status_color = if ca_exists { ACCENT_GREEN } else { ACCENT_RED };

                ui.horizontal(|ui| {
                    ui.label(RichText::new("CA Store Path:").size(11.0).color(TEXT_1).strong());
                    ui.label(
                        RichText::new(ca_path.to_string_lossy())
                            .size(11.0)
                            .color(TEXT_0)
                            .family(FontFamily::Monospace)
                    );
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(status_icon)
                            .size(11.0)
                            .color(status_color)
                            .strong()
                    );
                });

                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    // Button 1: Export CA Certificate (.crt)
                    if ui.add(
                        egui::Button::new(RichText::new("📥 Export CA Certificate (.crt)").size(12.0).color(TEXT_0).strong())
                            .fill(ACCENT_BLUE)
                            .rounding(Rounding::same(4.0))
                    ).clicked() {
                        if let Some(dest_path) = rfd::FileDialog::new()
                            .set_file_name("ajproxy_ca.crt")
                            .add_filter("Certificate", &["crt", "pem"])
                            .save_file()
                        {
                            match cert::export_ca_cert(&dest_path) {
                                Ok(_) => {
                                    settings.cert_status_msg = format!("✅ CA Certificate exported successfully to {}", dest_path.display());
                                }
                                Err(e) => {
                                    settings.cert_status_msg = format!("❌ Export error: {}", e);
                                }
                            }
                        }
                    }

                    ui.add_space(8.0);

                    // Button 2: Regenerate CA Certificate
                    if ui.add(
                        egui::Button::new(RichText::new("🔄 Regenerate CA Certificate").size(12.0).color(TEXT_0).strong())
                            .fill(BG_RAISED)
                            .stroke(Stroke::new(1.0_f32, ACCENT_AMBER))
                            .rounding(Rounding::same(4.0))
                    ).clicked() {
                        match cert::generate_and_save_ca() {
                            Ok(_) => {
                                settings.cert_status_msg = format!("✅ New Root CA generated & saved to {}", cert::get_cert_dir().display());
                            }
                            Err(e) => {
                                settings.cert_status_msg = format!("❌ Regeneration error: {}", e);
                            }
                        }
                    }

                    ui.add_space(8.0);

                    // Button 3: Auto-Trust CA System-Wide
                    if ui.add(
                        egui::Button::new(RichText::new("⚡ Auto-Trust CA System-Wide (Ubuntu)").size(12.0).color(TEXT_0).strong())
                            .fill(ACCENT_GREEN)
                            .rounding(Rounding::same(4.0))
                    ).clicked() {
                        match cert::install_ca_system_wide() {
                            Ok(msg) => {
                                settings.cert_status_msg = msg;
                            }
                            Err(e) => {
                                settings.cert_status_msg = format!("❌ Auto-trust error: {}", e);
                            }
                        }
                    }

                    ui.add_space(8.0);

                    // Button 3: Upload / Import CA Certificate
                    if ui.add(
                        egui::Button::new(RichText::new("📤 Upload / Import CA Certificate").size(12.0).color(TEXT_0).strong())
                            .fill(BG_RAISED)
                            .stroke(Stroke::new(1.0_f32, BORDER))
                            .rounding(Rounding::same(4.0))
                    ).clicked() {
                        if let Some(src_path) = rfd::FileDialog::new()
                            .add_filter("Certificate File", &["crt", "pem", "cer", "key"])
                            .pick_file()
                        {
                            match cert::import_ca_cert(&src_path) {
                                Ok(_) => {
                                    settings.cert_status_msg = format!("✅ Custom CA Certificate imported & activated from {}", src_path.display());
                                }
                                Err(e) => {
                                    settings.cert_status_msg = format!("❌ Import error: {}", e);
                                }
                            }
                        }
                    }
                });

                if !settings.cert_status_msg.is_empty() {
                    ui.add_space(8.0);
                    let is_err = settings.cert_status_msg.contains("❌");
                    let msg_color = if is_err { ACCENT_RED } else { ACCENT_GREEN };

                    ui.label(
                        RichText::new(&settings.cert_status_msg)
                            .size(11.0)
                            .color(msg_color)
                            .strong()
                            .family(FontFamily::Monospace)
                    );
                }
            });

            ui.add_space(8.0);

            // ── Section 3: Traffic Noise & Asset Filtering ────────────────────
            section_frame().show(ui, |ui| {
                ui.label(RichText::new("Traffic Noise & Asset Filtering").size(13.0).color(TEXT_0).strong());
                ui.label(RichText::new("Hide non-essential background assets, static scripts, images, and telemetry from Dashboard, HTTP History, and Intercept.").size(11.0).color(TEXT_2));
                ui.separator();
                ui.add_space(4.0);

                ui.checkbox(
                    &mut settings.filter_scripts_styles_fonts,
                    "Filter CSS, JS & Fonts (.css, .js, .woff, .woff2, .ttf | text/css, font/*, javascript)",
                );
                ui.add_space(3.0);

                ui.checkbox(
                    &mut settings.filter_images_media,
                    "Filter Images & Media (.png, .jpg, .jpeg, .gif, .svg, .ico | image/*)",
                );
                ui.add_space(3.0);

                ui.checkbox(
                    &mut settings.filter_noisy_domains,
                    "Filter Cloudflare & Google Noisy Domains (challenges.cloudflare.com, *.google.com)",
                );
            });

            ui.add_space(8.0);

            // ── Section 4: Global Match & Replace Engine ────────────────────────
            section_frame().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Global Match & Replace Engine").size(13.0).color(TEXT_0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(
                            egui::Button::new(RichText::new("➕ Add Match & Replace Rule").size(11.0).color(TEXT_0).strong())
                                .fill(ACCENT_BLUE)
                                .rounding(Rounding::same(4.0))
                        ).clicked() {
                            settings.match_rules.push(InterceptRule {
                                enabled: true,
                                match_type: "Header".into(),
                                pattern: "".into(),
                                action: "".into(),
                            });
                        }
                    });
                });
                ui.label(RichText::new("Automatically rewrite matching request/response headers, paths, or body content on the fly across all proxy traffic.").size(11.0).color(TEXT_2));
                ui.separator();
                ui.add_space(4.0);

                if settings.match_rules.is_empty() {
                    ui.add_space(8.0);
                    ui.label(RichText::new("No Match & Replace rules configured. Click '+ Add Match & Replace Rule' above to create one.").size(11.0).color(TEXT_2));
                    ui.add_space(8.0);
                } else {
                    let mut to_delete = None;
                    let mut changed = false;

                    for (idx, rule) in settings.match_rules.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            if ui.checkbox(&mut rule.enabled, "").changed() {
                                changed = true;
                            }

                            // Match Scope Combo
                            egui::ComboBox::from_id_source(format!("match_type_combo_{}", idx))
                                .selected_text(&rule.match_type)
                                .show_ui(ui, |ui| {
                                    if ui.selectable_value(&mut rule.match_type, "Header".into(), "Header").changed() { changed = true; }
                                    if ui.selectable_value(&mut rule.match_type, "Request Body".into(), "Request Body").changed() { changed = true; }
                                    if ui.selectable_value(&mut rule.match_type, "URL / Path".into(), "URL / Path").changed() { changed = true; }
                                    if ui.selectable_value(&mut rule.match_type, "Anywhere".into(), "Anywhere").changed() { changed = true; }
                                });

                            ui.label(RichText::new("Match:").size(11.0).color(TEXT_1).strong());
                            if ui.add(egui::TextEdit::singleline(&mut rule.pattern).hint_text("Pattern to find...").desired_width(170.0)).changed() {
                                changed = true;
                            }

                            ui.label(RichText::new("Replace:").size(11.0).color(TEXT_1).strong());
                            if ui.add(egui::TextEdit::singleline(&mut rule.action).hint_text("Replacement text...").desired_width(170.0)).changed() {
                                changed = true;
                            }

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.add(
                                    egui::Button::new(RichText::new("✖").size(11.0).color(ACCENT_RED))
                                        .fill(BG_RAISED)
                                        .stroke(Stroke::new(0.5_f32, BORDER))
                                ).clicked() {
                                    to_delete = Some(idx);
                                }
                            });
                        });
                        ui.add_space(3.0);
                    }

                    if let Some(idx) = to_delete {
                        settings.match_rules.remove(idx);
                        changed = true;
                    }

                    if changed {
                        crate::proxy::listener::update_match_rules(settings.match_rules.clone());
                    }
                }
            });

            ui.add_space(8.0);

            // ── Section 5: Automated Header Injection Engine ───────────────────
            section_frame().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Automated Header Injection Engine").size(13.0).color(TEXT_0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(
                            egui::Button::new(RichText::new("➕ Add Header Rule").size(11.0).color(TEXT_0).strong())
                                .fill(ACCENT_BLUE)
                                .rounding(Rounding::same(4.0))
                        ).clicked() {
                            settings.header_injection_rules.push(HeaderInjectionRule {
                                enabled: true,
                                scope: "*".into(),
                                header_name: "".into(),
                                header_value: "".into(),
                            });
                        }
                    });
                });
                ui.label(RichText::new("Automatically inject custom headers (e.g. X-Forwarded-For, X-Bounty-Key, Authorization) into outgoing requests.").size(11.0).color(TEXT_2));
                ui.separator();
                ui.add_space(4.0);

                // Scope Info Callout
                egui::Frame::none()
                    .fill(BG_RAISED)
                    .rounding(Rounding::same(4.0))
                    .inner_margin(egui::Margin::same(6.0))
                    .show(ui, |ui| {
                        ui.label(RichText::new("💡 Scope / Target Filter Guide:").size(11.0).color(ACCENT_CYAN).strong());
                        ui.label(RichText::new("• Type '*' to inject this header into ALL outgoing requests across all domains.").size(10.0).color(TEXT_1));
                        ui.label(RichText::new("• Type a host keyword (e.g. 'target' or 'api.example.com') to inject only when request host matches.").size(10.0).color(TEXT_1));
                    });

                ui.add_space(6.0);

                if settings.header_injection_rules.is_empty() {
                    ui.add_space(4.0);
                    ui.label(RichText::new("No header injection rules configured. Click '+ Add Header Rule' above to create one.").size(11.0).color(TEXT_2));
                    ui.add_space(6.0);
                } else {
                    let mut to_delete = None;
                    let mut changed = false;

                    for (idx, rule) in settings.header_injection_rules.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            if ui.checkbox(&mut rule.enabled, "").changed() {
                                changed = true;
                            }

                            ui.label(RichText::new("Target / Scope:").size(11.0).color(TEXT_1).strong());
                            if ui.add(egui::TextEdit::singleline(&mut rule.scope).hint_text("* or domain...").desired_width(110.0)).changed() {
                                changed = true;
                            }

                            ui.label(RichText::new("Header Name:").size(11.0).color(TEXT_1).strong());
                            if ui.add(egui::TextEdit::singleline(&mut rule.header_name).hint_text("X-Bounty-Key...").desired_width(140.0)).changed() {
                                changed = true;
                            }

                            ui.label(RichText::new("Value:").size(11.0).color(TEXT_1).strong());
                            if ui.add(egui::TextEdit::singleline(&mut rule.header_value).hint_text("Header value...").desired_width(170.0)).changed() {
                                changed = true;
                            }

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.add(
                                    egui::Button::new(RichText::new("✖").size(11.0).color(ACCENT_RED))
                                        .fill(BG_RAISED)
                                        .stroke(Stroke::new(0.5_f32, BORDER))
                                ).clicked() {
                                    to_delete = Some(idx);
                                }
                            });
                        });
                        ui.add_space(3.0);
                    }

                    if let Some(idx) = to_delete {
                        settings.header_injection_rules.remove(idx);
                        changed = true;
                    }

                    if changed {
                        crate::proxy::listener::update_header_injection_rules(settings.header_injection_rules.clone());
                    }
                }
            });
        });
}